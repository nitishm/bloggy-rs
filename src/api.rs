use std::sync::Arc;

use chrono::Utc;
use kube::{
    api::{Patch, PatchParams},
    runtime::controller::Action,
    Api, CustomResource, ResourceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::*;

use crate::{Context, Error};

#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[kube(
    group = "letsgopherit.com",
    version = "v1",
    kind = "Blog",
    plural = "blogs",
    derive = "PartialEq",
    status = "BlogStatus",
    shortname = "blog"
)]
pub struct BlogSpec {
    author: Vec<String>,
    title: String,
    draft: bool,
    content: String,
}

impl BlogSpec {
    pub fn is_draft(&self) -> bool {
        self.draft
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub struct BlogStatus {
    draft: bool,
    created_at: String,
    published_at: String,
    modified_at: String,
}

impl Blog {
    pub async fn reconcile(&self, ctx: Arc<Context>) -> Result<Action, kube::Error> {
        let client = ctx.client.clone();
        let name = self.name_any();
        let blogs: Api<Blog> = Api::all(client);

        let now = Utc::now().to_string();
        let mut published_at: String = now.clone();

        if self.spec.is_draft() {
            published_at = "".to_string();
        }

        // do something with the blog object
        let pp = PatchParams::apply("ctrlr").force();
        let patch = Patch::Apply(json!({
        "apiVersion": "letsgopherit.com/v1",
        "kind": "Blog",
        "status":  BlogStatus {
            draft: self.spec.is_draft(),
            created_at: now.clone(),
            published_at: published_at.clone(),
            modified_at: now.clone(),
        },
        }));

        let _o = blogs.patch_status(&name, &pp, &patch).await?;

        // If no events were received, check back every 5 minutes
        Ok(Action::requeue(std::time::Duration::from_secs(5 * 60)))
    }

    pub async fn cleanup(&self, ctx: Arc<Context>) -> Result<Action, kube::Error> {
        info!("Cleaning up Blog \"{}\"", self.name_any());
        Ok(Action::await_change())
    }
}
