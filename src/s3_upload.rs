//! S3 things
use anyhow::Result;
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::{Client, Config};
use aws_types::region::Region;
use log::*;

use crate::config::Configuration;

#[derive(Debug)]
#[allow(dead_code)]
pub enum S3Result {
    DeleteFailure(String),
    // DownloadFailure(String),
    FileOpenFail(String),
    HeadError(String),
    Success,
    UploadFailure(String),
    FileNotFound,
}

pub struct S3Client {
    client: Client,
    bucket: String,
}

impl S3Client {
    /// get you a client with a default config file
    pub fn try_new() -> anyhow::Result<Self> {
        let config: crate::config::Configuration = Configuration::try_new()?;
        Ok(Self::from(config))
    }

    /// Loaded the config already? Get an S3 client.
    pub fn from(config: Configuration) -> Self {
        let creds = Credentials::new(
            config.s3_access_key_id,
            config.s3_secret_access_key,
            None,
            None,
            "memetool",
        );

        debug!("S3 Creds: {:?}", creds);

        let mut client_config = Config::builder()
            .credentials_provider(creds)
            .force_path_style(true)
            .region(Region::new(config.s3_region));
        // set the endpoint if we need to
        if let Some(endpoint_uri) = config.s3_endpoint {
            info!("Setting s3 endpoint: {} ", endpoint_uri);
            client_config = client_config.endpoint_url(endpoint_uri);
        };
        let client = Client::from_conf(client_config.build());
        debug!("s3 client config: {:?}", client);

        Self {
            client,
            bucket: config.s3_bucket,
        }
    }

    pub async fn head_object(&self, key: &str) -> Result<String, S3Result> {
        eprintln!("head_object: {}", key);
        let head = self
            .client
            .head_object()
            .key(key)
            .bucket(&self.bucket)
            .send()
            .await;

        match head {
            // TODO Reduced struct for nicer data
            Ok(response) => Ok(format!("{:?}", response)),
            Err(error) => {
                match error {
                    aws_sdk_s3::error::SdkError::ConstructionFailure(err) => Err(
                        S3Result::HeadError(format!("ConstructionFailure: {:?}", err)),
                    ),
                    aws_sdk_s3::error::SdkError::TimeoutError(err) => {
                        Err(S3Result::HeadError(format!("TimeoutError: {:?}", err)))
                    }
                    aws_sdk_s3::error::SdkError::DispatchFailure(err) => {
                        Err(S3Result::HeadError(format!("DispatchFailure: {:?}", err)))
                    }
                    aws_sdk_s3::error::SdkError::ResponseError(err) => {
                        Err(S3Result::HeadError(format!("ResponseError: {:?}", err)))
                    }
                    aws_sdk_s3::error::SdkError::ServiceError(service_error) => {
                        match service_error.into_err() {
                            aws_sdk_s3::operation::head_object::HeadObjectError::NotFound(_) => {
                                Err(S3Result::FileNotFound)
                            }
                            // aws_sdk_s3::operation::head_object::HeadObjectError::Unhandled(err) => {
                            //     Err(S3Result::HeadError(format!("ResponseError: {:?}", err)))
                            // }
                            _ => todo!(),
                        }
                    }
                    _ => Err(S3Result::HeadError("Generic Error".to_string())),
                }
                // println!("Error doing head: {:?}", error);
                // Err(S3Result::HeadError(format!(
                //     "Failed head_object() file: {:?}",
                //     error
                // )))
            }
        }
    }
    pub async fn put_object(&self, key: &str, filename: &str) -> Result<String, S3Result> {
        eprintln!("put_object: {} => {}", filename, key);
        let bytestream = match ByteStream::from_path(&filename).await {
            Ok(value) => value,
            Err(error) => {
                return Err(S3Result::FileOpenFail(format!(
                    "Failed to open file: {:?}",
                    error
                )))
            }
        };

        let upload = self
            .client
            .put_object()
            .key(key)
            .bucket(&self.bucket)
            .body(bytestream)
            .send()
            .await;

        match upload {
            Ok(response) => Ok(format!("{:?}", response)),
            Err(error) => Err(S3Result::UploadFailure(format!(
                "Failed to upload file: {:?}",
                error
            ))),
        }
    }
}
