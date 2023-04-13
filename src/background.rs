//! Background processing things
//!
//!
//!

use std::path::PathBuf;
use std::sync::Arc;

use log::*;
use tokio::sync::mpsc;

use crate::image_utils::load_image_to_thumbnail_async;
use crate::{AppMsg, ThumbImageMsg};

pub async fn background(mut rx: mpsc::Receiver<AppMsg>, tx: mpsc::Sender<AppMsg>) {
    info!("Background thread started");
    while let Some(msg) = rx.recv().await {
        debug!("Background received message: {:?}", msg);
        let response = match msg {
            AppMsg::LoadImage(msg) => {
                let filepath = msg.filepath;
                match load_image_to_thumbnail_async(&PathBuf::from(filepath.clone()), None).await {
                    Ok(image) => AppMsg::ThumbImageResponse(ThumbImageMsg {
                        filepath,
                        page: msg.page,
                        image: Some(Arc::new(image)),
                    }),
                    Err(error) => {
                        error!("Failed to load {} {}", filepath, error);
                        AppMsg::ImageLoadFailed {
                            filename: filepath.to_string(),
                            error,
                        }
                    }
                }
            }
            AppMsg::ThumbImageResponse(_) => todo!(),
            AppMsg::ImageLoadFailed {
                filename: _,
                error: _,
            } => todo!(),
            AppMsg::NewAppState(xxx) => AppMsg::NewAppState(xxx),
            AppMsg::Echo(_) => todo!(),
        };

        // ctx.request_repaint_after(Duration::from_millis(500));

        if let Err(err) = tx.send(response).await {
            error!("Background failed to send echo! {}", err.to_string());
        }
    }
}
