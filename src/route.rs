use std::{process::Stdio, time::Duration};

use actix_files::NamedFile;
use actix_web::{get, post, web, Responder, Scope};
use log::{error, info};
use serde::{Deserialize, Serialize};

use crate::{sam::SAMPool, util::log_ise, State};
pub fn make_scope() -> Scope {
    return web::scope("")
        .service(index)
        .service(draw_get)
        .service(draw_post);
}

#[get("/")]
pub async fn index(state: State) -> impl Responder {
    NamedFile::open_async(state.web_dir.join("index.html")).await
}

#[derive(Deserialize)]
pub struct Req {
    string: String,
}

#[derive(Serialize)]
pub struct Resp<T: Serialize> {
    code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
}

pub fn ok<T: Serialize>(d: Option<T>) -> actix_web::Result<web::Json<Resp<T>>> {
    Ok(web::Json(Resp {
        code: 0,
        message: None,
        data: d,
    }))
}
pub fn err<T: Into<String>, P: Serialize>(e: T) -> actix_web::Result<web::Json<Resp<P>>> {
    Ok(web::Json(Resp {
        code: -1,
        message: Some(e.into()),
        data: None,
    }))
}

#[get("/draw")]
pub async fn draw_get(
    state: State,
    req: web::Query<Req>,
) -> actix_web::Result<web::Json<Resp<String>>> {
    generate_graph(state, req.string.as_str()).await
}

#[post("/draw")]
pub async fn draw_post(
    state: State,
    req: web::Form<Req>,
) -> actix_web::Result<web::Json<Resp<String>>> {
    generate_graph(state, req.string.as_str()).await
}

pub async fn generate_graph(
    state: State,
    string: &str,
) -> actix_web::Result<web::Json<Resp<String>>> {
    let chrs: Vec<_> = string.chars().collect();
    if chrs.len() > state.config.max_len as usize {
        return err("字符串过长!");
    }
    let _permit = if let Ok(v) = state.semaphore.try_acquire() {
        v
    } else {
        return err("当前用户过多，请稍后再试！");
    };

    info!("Generating SAM: {}", string);
    // let config = self.config.as_ref().unwrap();
    let text_cloned = string.to_string();
    let dot_data = tokio::task::spawn_blocking(move || {
        let mut pool = SAMPool::default();
        for (id, s) in text_cloned.split("|").enumerate() {
            pool.join_string(s, id as i32);
        }
        pool.collect();
        pool.generate_graph()
    })
    .await
    .map_err(log_ise)?;
    info!("Generation done!");
    let work_dir = tempfile::tempdir()?;
    let dot_file = work_dir.path().join("out.dot");
    let svg_file = work_dir.path().join("out.svg");
    tokio::fs::write(dot_file.clone(), dot_data).await?;
    let mut process = tokio::process::Command::new(state.config.dot_executable.clone())
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stdin(Stdio::null())
        .arg("-Tsvg")
        .args(&["-o", svg_file.to_str().unwrap()])
        .arg(dot_file.to_str().unwrap())
        .spawn()?;
    info!("Rendering svg to {:?}, using {:?}", svg_file, dot_file);
    match tokio::time::timeout(
        Duration::from_millis(state.config.render_timeout),
        process.wait(),
    )
    .await
    {
        Err(e) => {
            process.kill().await?;
            return err(format!("dot执行超时: {}", e));
        }
        Ok(ret) => {
            let exit_status = ret?;
            if !exit_status.success() {
                error!("{}", exit_status);
                return Err(log_ise(format!("执行dot失败!\n{}", exit_status)));
            } else {
                let svg_data = tokio::fs::read_to_string(svg_file).await?;
                return ok(Some(svg_data));
            }
        }
    };
}
