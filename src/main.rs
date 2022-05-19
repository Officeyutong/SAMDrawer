use std::{path::PathBuf, sync::Arc};

use actix_web::{web, App, HttpServer};
use anyhow::anyhow;
use config::Config;
use flexi_logger::{
    DeferredNow, Duplicate, FileSpec, Logger, Record, TS_DASHES_BLANK_COLONS_DOT_BLANK,
};
use log::info;
mod config;
mod route;
mod sam;
mod util;
pub type ResultType<T> = anyhow::Result<T>;

pub struct AppState {
    pub config: Arc<Config>,
    pub base_dir: PathBuf,
    pub web_dir: PathBuf,
    pub semaphore: tokio::sync::Semaphore,
}
pub type State = web::Data<AppState>;

pub fn my_log_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(
        w,
        "[{}] {} [{}:{}] {}",
        now.format(TS_DASHES_BLANK_COLONS_DOT_BLANK),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        record.line().unwrap_or(0),
        &record.args()
    )
}
#[tokio::main]
async fn main() -> ResultType<()> {
    if !std::path::Path::new("config.yaml").exists() {
        tokio::fs::write(
            "config.yaml",
            serde_yaml::to_string(&Config::default())?.as_bytes(),
        )
        .await?;
        return Err(anyhow!("已创建默认配置文件，请修改后重启。"));
    }
    let config = Arc::new(
        serde_yaml::from_str::<Config>(
            &tokio::fs::read_to_string("config.yaml")
                .await
                .map_err(|e| anyhow!("读取配置文件错误: {}", e))?,
        )
        .map_err(|e| anyhow!("反序列化错误: {}", e))?,
    );
    Logger::try_with_str(&config.log_level)
        .map_err(|_| anyhow!("Invalid loggine level: {}", config.log_level))?
        .format(my_log_format)
        .log_to_file(FileSpec::default().directory("logs").basename("sam"))
        .duplicate_to_stdout(Duplicate::All)
        .start()
        .map_err(|e| anyhow!("Failed to start logger!\n{}", e))?;
    let base_dir = std::env::current_dir().unwrap();
    let app_state = web::Data::new(AppState {
        config: config.clone(),
        base_dir: base_dir.clone(),
        web_dir: base_dir.as_path().join("web"),
        semaphore: tokio::sync::Semaphore::new(config.max_count_sametime),
    });
    {
        let web_dir = app_state.web_dir.as_path();
        if !web_dir.exists() {
            info!("Missing web directory, creating..");
            std::fs::create_dir(web_dir)?;
        }
        let index_html = web_dir.join("index.html");
        if !index_html.exists() {
            let index_bytes = include_bytes!("index.html");
            tokio::fs::write(index_html, index_bytes).await?;
        }
    }
    let bind = (config.bind_host.clone(), config.bind_port);
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(actix_web::middleware::Logger::new(
                r#"%a,%{r}a "%r" %s %b %T"#,
            ))
            .service(route::make_scope())
    })
    .bind(bind)?
    .run()
    .await?;
    return Ok(());
}
