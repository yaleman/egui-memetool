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
            AppMsg::UploadAborted(_) => panic!("Frontend shouldn't send aborted upload message"),
            AppMsg::UploadImage(filepath) => {
                debug!("Starting S3 Upload!");
                match crate::s3_upload::S3Client::try_new() {
                    Ok(s3_client) => {
                        let key = filepath.split('/').last().unwrap();
                        match s3_client.head_object(key).await {
                            Ok(val) => {
                                info!("File already exists in S3: {:?}", val);
                                AppMsg::UploadAborted(format!("File Exists in s3: {:?}", val))
                            }
                            Err(err) => {
                                if let crate::s3_upload::S3Result::FileNotFound = err {
                                    // we didn't find the file
                                    debug!("Uploading {} to S3", filepath);
                                    match s3_client.put_object(key, &filepath).await {
                                        Err(err) => AppMsg::Error(format!("{:?}", err)),
                                        // panic!("Failed to upload {} {:?}", filepath, err);
                                        Ok(_) => {
                                            info!("Successfully uploaded {} to S3", filepath);
                                            AppMsg::UploadComplete(filepath)
                                        }
                                    }
                                } else {
                                    AppMsg::Error(format!(
                                        "Failed to check existence of file in S3: {err:?}"
                                    ))
                                }
                            }
                        }
                    }
                    Err(err) => {
                        AppMsg::UploadAborted(format!("Failed to create S3 Client: {:?}", err))
                    }
                }
            }
            AppMsg::UploadComplete(filepath) => {
                panic!("The frontend sent UploadComplete({filepath})");
            }
            AppMsg::Error(err) => {
                AppMsg::Error(format!("The frontend sent Error({err}) to the backend!"))
            }
        };

        // ctx.request_repaint_after(Duration::from_millis(500));

        if let Err(err) = tx.send(response).await {
            error!("Background failed to send echo! {}", err.to_string());
        }
    }
}
