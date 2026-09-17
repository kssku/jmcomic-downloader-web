use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes256;
use anyhow::{anyhow, Context};
use base64::engine::general_purpose;
use base64::Engine;
use bytes::Bytes;
use image::ImageFormat;
use parking_lot::RwLock;
use reqwest::cookie::Jar;
use reqwest::StatusCode;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::{Jitter, RetryTransientMiddleware};
use serde_json::json;

use crate::context::AppContext;
use crate::extensions::{AppContextExt, ClientBuilderExt};
use crate::responses::{
    GetChapterRespData, GetComicRespData, GetFavoriteRespData, GetUserProfileRespData,
    JmResp, RedirectRespData, SearchResp, SearchRespData,
};
use crate::types::{FavoriteSort, SearchSort};
use crate::utils;

/// jm APP 签名密钥。
const APP_TOKEN_SECRET: &str = "18comicAPP";
const APP_TOKEN_SECRET_2: &str = "18comicAPPContent";
const APP_DATA_SECRET: &str = "185Hcomic3PAPP7R";
const APP_VERSION: &str = "2.0.13";

/// jm 的 API 域名（禁漫会轮换域名，配置里选一个或自定义）。
pub const API_DOMAIN_1: &str = "www.cdnzack.cc";
pub const API_DOMAIN_2: &str = "www.cdnhth.cc";
pub const API_DOMAIN_3: &str = "www.cdnhth.net";
pub const API_DOMAIN_4: &str = "www.cdnbea.net";
pub const API_DOMAIN_5: &str = "www.cdn-mspjmapiproxy.xyz";

/// jm 图片 CDN 域名。
pub const IMAGE_DOMAIN: &str = "cdn-msp2.jmapiproxy2.cc";

/// 请求超时（与 pica 一致，放宽以吸收抖动）。
const API_REQUEST_TIMEOUT_SECS: u64 = 15;
const API_RETRY_TOTAL_SECS: u64 = 30;

#[derive(Debug, Clone, PartialEq)]
enum ApiPath {
    Login,
    GetUserProfile,
    Search,
    GetComic,
    GetChapter,
    GetScrambleId,
    GetFavoriteFolder,
}

impl ApiPath {
    fn as_str(&self) -> &'static str {
        match self {
            // 获取用户信息也用 /login（带 AVS cookie 时）
            ApiPath::Login | ApiPath::GetUserProfile => "/login",
            ApiPath::Search => "/search",
            ApiPath::GetComic => "/album",
            ApiPath::GetChapter => "/chapter",
            ApiPath::GetScrambleId => "/chapter_view_template",
            ApiPath::GetFavoriteFolder => "/favorite",
        }
    }
}

#[derive(Clone)]
pub struct JmClient {
    app: AppContext,
    api_client: Arc<RwLock<ClientWithMiddleware>>,
    api_jar: Arc<Jar>,
    img_client: Arc<RwLock<ClientWithMiddleware>>,
}
impl JmClient {
    pub fn new(app: AppContext) -> Self {
        let api_jar = Arc::new(Jar::default());
        let api_client = create_api_client(&app, &api_jar);
        let api_client = Arc::new(RwLock::new(api_client));
        let img_client = create_img_client(&app);
        let img_client = Arc::new(RwLock::new(img_client));
        Self {
            app,
            api_client,
            api_jar,
            img_client,
        }
    }

    /// 配置变更（代理/域名）后重建客户端。
    pub fn reload_client(&self) {
        let api_client = create_api_client(&self.app, &self.api_jar);
        *self.api_client.write() = api_client;
        let img_client = create_img_client(&self.app);
        *self.img_client.write() = img_client;
    }

    fn api_domain(&self) -> String {
        self.app.get_config().read().api_base_url.clone()
    }

    /// 当前使用的 API 域名（供前端/日志展示）。
    pub fn base_url(&self) -> String {
        self.api_domain()
    }

    async fn jm_request(
        &self,
        method: reqwest::Method,
        path: ApiPath,
        query: Option<serde_json::Value>,
        form: Option<serde_json::Value>,
        ts: u64,
    ) -> anyhow::Result<reqwest::Response> {
        let tokenparam = format!("{ts},{APP_VERSION}");
        let token = if path == ApiPath::GetScrambleId {
            utils::md5_hex(&format!("{ts}{APP_TOKEN_SECRET_2}"))
        } else {
            utils::md5_hex(&format!("{ts}{APP_TOKEN_SECRET}"))
        };

        let api_domain = self.api_domain();
        let path = path.as_str();
        let request = self
            .api_client
            .read()
            .request(method, format!("https://{api_domain}{path}").as_str())
            .header("token", token)
            .header("tokenparam", tokenparam)
            .header(
                "user-agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
            );

        let http_resp = match form {
            Some(payload) => request.query(&query).form(&payload).send().await,
            None => request.query(&query).send().await,
        }
        .map_err(|e| {
            if e.is_timeout() {
                anyhow::Error::from(e).context("连接超时，请使用代理或换条线路重试")
            } else {
                anyhow::Error::from(e)
            }
        })?;

        Ok(http_resp)
    }

    async fn jm_get(
        &self,
        path: ApiPath,
        query: Option<serde_json::Value>,
        ts: u64,
    ) -> anyhow::Result<reqwest::Response> {
        self.jm_request(reqwest::Method::GET, path, query, None, ts)
            .await
    }

    async fn jm_post(
        &self,
        path: ApiPath,
        query: Option<serde_json::Value>,
        payload: Option<serde_json::Value>,
        ts: u64,
    ) -> anyhow::Result<reqwest::Response> {
        self.jm_request(reqwest::Method::POST, path, query, payload, ts)
            .await
    }
    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> anyhow::Result<GetUserProfileRespData> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let form = json!({"username": username, "password": password});
        let http_resp = self.jm_post(ApiPath::Login, None, Some(form), ts).await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("使用账号密码登录失败，预料之外的状态码({status}): {body}"));
        }
        let jm_resp = serde_json::from_str::<JmResp>(&body)
            .context(format!("将body解析为JmResp失败: {body}"))?;
        if jm_resp.code != 200 {
            return Err(anyhow!("使用账号密码登录失败，预料之外的code: {jm_resp:?}"));
        }
        let data = jm_resp.data.as_str().context(format!(
            "使用账号密码登录失败，data字段不是字符串: {jm_resp:?}"
        ))?;
        let data = decrypt_data(ts, data)?;
        let mut user_profile = serde_json::from_str::<GetUserProfileRespData>(&data)
            .context(format!("解密后的data解析为GetUserProfileRespData失败: {data}"))?;
        user_profile.photo = format!("https://{IMAGE_DOMAIN}/media/users/{}", user_profile.photo);
        Ok(user_profile)
    }

    pub async fn get_user_profile(&self) -> anyhow::Result<GetUserProfileRespData> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let http_resp = self.jm_post(ApiPath::GetUserProfile, None, None, ts).await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status == StatusCode::UNAUTHORIZED {
            return Err(anyhow!("获取用户信息失败，Cookie无效或已过期，请重新登录"));
        } else if status != StatusCode::OK {
            return Err(anyhow!("获取用户信息失败，预料之外的状态码({status}): {body}"));
        }
        let jm_resp = serde_json::from_str::<JmResp>(&body)
            .context(format!("将body解析为JmResp失败: {body}"))?;
        if jm_resp.code != 200 {
            return Err(anyhow!("获取用户信息失败，预料之外的code: {jm_resp:?}"));
        }
        let data = jm_resp
            .data
            .as_str()
            .context(format!("获取用户信息失败，data字段不是字符串: {jm_resp:?}"))?;
        let data = decrypt_data(ts, data)?;
        let mut user_profile = serde_json::from_str::<GetUserProfileRespData>(&data)
            .context(format!("解密后的data解析为GetUserProfileRespData失败: {data}"))?;
        user_profile.photo = format!("https://{IMAGE_DOMAIN}/media/users/{}", user_profile.photo);
        Ok(user_profile)
    }

    pub async fn search(
        &self,
        keyword: &str,
        page: i64,
        sort: SearchSort,
    ) -> anyhow::Result<SearchResp> {
        let query = json!({
            "main_tag": 0,
            "search_query": keyword,
            "page": page,
            "o": sort.as_str(),
        });
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let http_resp = self.jm_get(ApiPath::Search, Some(query), ts).await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("搜索失败，预料之外的状态码({status}): {body}"));
        }
        let jm_resp = serde_json::from_str::<JmResp>(&body)
            .context(format!("将body解析为JmResp失败: {body}"))?;
        if jm_resp.code != 200 {
            return Err(anyhow!("搜索失败，预料之外的code: {jm_resp:?}"));
        }
        let data = jm_resp
            .data
            .as_str()
            .context(format!("搜索失败，data字段不是字符串: {jm_resp:?}"))?;
        let data = decrypt_data(ts, data)?;
        // 搜索命中单个漫画时会返回 redirect
        if let Ok(redirect_resp_data) = serde_json::from_str::<RedirectRespData>(&data) {
            let comic_resp_data = self
                .get_comic(redirect_resp_data.redirect_aid.parse()?)
                .await?;
            return Ok(SearchResp::ComicRespData(Box::new(comic_resp_data)));
        }
        if let Ok(search_resp_data) = serde_json::from_str::<SearchRespData>(&data) {
            return Ok(SearchResp::SearchRespData(search_resp_data));
        }
        Err(anyhow!(
            "将解密后的数据解析为SearchRespData或RedirectRespData失败: {data}"
        ))
    }

    pub async fn get_comic(&self, aid: i64) -> anyhow::Result<GetComicRespData> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let query = json!({"id": aid});
        let http_resp = self.jm_get(ApiPath::GetComic, Some(query), ts).await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("获取漫画失败，预料之外的状态码({status}): {body}"));
        }
        let jm_resp = serde_json::from_str::<JmResp>(&body)
            .context(format!("将body解析为JmResp失败: {body}"))?;
        if jm_resp.code != 200 {
            return Err(anyhow!("获取漫画失败，预料之外的code: {jm_resp:?}"));
        }
        let data = jm_resp
            .data
            .as_str()
            .context(format!("获取漫画失败，data字段不是字符串: {jm_resp:?}"))?;
        let data = decrypt_data(ts, data)?;
        let comic = serde_json::from_str::<GetComicRespData>(&data)
            .context(format!("解密后的data解析为GetComicRespData失败: {data}"))?;
        Ok(comic)
    }

    pub async fn get_chapter(&self, id: i64) -> anyhow::Result<GetChapterRespData> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let query = json!({"id": id});
        let http_resp = self.jm_get(ApiPath::GetChapter, Some(query), ts).await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("获取章节失败，预料之外的状态码({status}): {body}"));
        }
        let jm_resp = serde_json::from_str::<JmResp>(&body)
            .context(format!("将body解析为JmResp失败: {body}"))?;
        if jm_resp.code != 200 {
            return Err(anyhow!("获取章节失败，预料之外的code: {jm_resp:?}"));
        }
        let data = jm_resp
            .data
            .as_str()
            .context(format!("获取章节失败，data字段不是字符串: {jm_resp:?}"))?;
        let data = decrypt_data(ts, data)?;
        let chapter = serde_json::from_str::<GetChapterRespData>(&data)
            .context(format!("解密后的data解析为GetChapterRespData失败: {data}"))?;
        Ok(chapter)
    }

    /// 获取章节的 scramble_id（用于图片还原）。
    pub async fn get_scramble_id(&self, id: i64) -> anyhow::Result<i64> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let query = json!({"id": id});
        let http_resp = self.jm_get(ApiPath::GetScrambleId, Some(query), ts).await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("获取scramble_id失败，预料之外的状态码({status}): {body}"));
        }
        let scramble_id = body
            .split("var scramble_id = ")
            .nth(1)
            .and_then(|s| s.split(';').next())
            .and_then(|s| s.trim().parse::<i64>().ok())
            .context(format!("从body中提取scramble_id失败: {body}"))?;
        Ok(scramble_id)
    }

    pub async fn get_favorite(
        &self,
        page: i64,
        sort: FavoriteSort,
    ) -> anyhow::Result<GetFavoriteRespData> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let query = json!({"page": page, "o": sort.as_str()});
        let http_resp = self.jm_get(ApiPath::GetFavoriteFolder, Some(query), ts).await?;
        let status = http_resp.status();
        let body = http_resp.text().await?;
        if status != StatusCode::OK {
            return Err(anyhow!("获取收藏夹失败，预料之外的状态码({status}): {body}"));
        }
        let jm_resp = serde_json::from_str::<JmResp>(&body)
            .context(format!("将body解析为JmResp失败: {body}"))?;
        if jm_resp.code != 200 {
            return Err(anyhow!("获取收藏夹失败，预料之外的code: {jm_resp:?}"));
        }
        let data = jm_resp
            .data
            .as_str()
            .context(format!("获取收藏夹失败，data字段不是字符串: {jm_resp:?}"))?;
        let data = decrypt_data(ts, data)?;
        let favorite = serde_json::from_str::<GetFavoriteRespData>(&data)
            .context(format!("解密后的data解析为GetFavoriteRespData失败: {data}"))?;
        Ok(favorite)
    }
    /// 下载图片原始数据（jm 图片可能是乱序的，还原在 download_manager 里做）。
    pub async fn get_img_data_and_format(&self, url: &str) -> anyhow::Result<(Bytes, ImageFormat)> {
        let request = self.img_client.read().get(url).header(
            "user-agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        );
        let http_resp = request.send().await?;
        let status = http_resp.status();
        if status != StatusCode::OK {
            let text = http_resp.text().await?;
            return Err(anyhow!("下载图片`{url}`失败，预料之外的状态码: {text}"));
        }
        let mut image_data = http_resp.bytes().await?;
        if image_data.is_empty() {
            // jm 缓存失效时返回空，带时间戳重试一次
            let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let query = json!({"ts": ts});
            let http_resp = self.img_client.read().get(url).query(&query).send().await?;
            let status = http_resp.status();
            if status != StatusCode::OK {
                let text = http_resp.text().await?;
                return Err(anyhow!("下载图片`{url}`失败，预料之外的状态码: {text}"));
            }
            image_data = http_resp.bytes().await?;
        }
        let format = image::guess_format(&image_data)
            .context("无法从图片数据中猜测出图片格式，可能图片数据不完整或已损坏")?;
        Ok((image_data, format))
    }
}

pub fn create_api_client(app: &AppContext, jar: &Arc<Jar>) -> ClientWithMiddleware {
    let builder = reqwest::ClientBuilder::new().cookie_provider(jar.clone());
    let builder = builder.set_proxy(app, "jm_api_client");
    let retry_policy = ExponentialBackoff::builder()
        .base(1)
        .jitter(Jitter::Bounded)
        .build_with_total_retry_duration(Duration::from_secs(API_RETRY_TOTAL_SECS));
    reqwest_middleware::ClientBuilder::new(
        builder.timeout(Duration::from_secs(API_REQUEST_TIMEOUT_SECS)).build().unwrap(),
    )
    .with(RetryTransientMiddleware::new_with_policy(retry_policy))
    .build()
}

pub fn create_img_client(app: &AppContext) -> ClientWithMiddleware {
    let builder = reqwest::ClientBuilder::new();
    let builder = builder.set_proxy(app, "jm_img_client");
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
    reqwest_middleware::ClientBuilder::new(builder.build().unwrap())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

/// 解密 jm 响应 data（AES-256-ECB，key = md5(ts + APP_DATA_SECRET)，输入 Base64）。
fn decrypt_data(ts: u64, data: &str) -> anyhow::Result<String> {
    let aes256_ecb_encrypted_data = general_purpose::STANDARD.decode(data)?;
    let key = utils::md5_hex(&format!("{ts}{APP_DATA_SECRET}"));
    let cipher = Aes256::new(GenericArray::from_slice(key.as_bytes()));
    let decrypted_data_with_padding: Vec<u8> = aes256_ecb_encrypted_data
        .chunks(16)
        .map(GenericArray::clone_from_slice)
        .flat_map(|mut block| {
            cipher.decrypt_block(&mut block);
            block.to_vec()
        })
        .collect();
    let padding_length = decrypted_data_with_padding
        .last()
        .copied()
        .context("解密数据为空")? as usize;
    if padding_length == 0 || padding_length > decrypted_data_with_padding.len() {
        return Err(anyhow!("PKCS#7 填充长度非法: {padding_length}"));
    }
    let decrypted_data_without_padding =
        decrypted_data_with_padding[..decrypted_data_with_padding.len() - padding_length].to_vec();
    let decrypted_data = String::from_utf8(decrypted_data_without_padding)?;
    Ok(decrypted_data)
}