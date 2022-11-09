use std::sync::Arc;
use std::time::Duration;

use crate::{Error, Result};
use chrono::prelude::*;
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use kube::api::ListParams;
use kube::runtime::finalizer::{finalizer, Event};
use kube::{
    api::{Patch, PatchParams},
    runtime::controller::{Action, Controller},
    Api, Client, CustomResource, ResourceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::*;
use tracing::*;

static BLOG_FINALIZER: &str = "blogs.letsgopherit.com";
static DEFAULT_NS: &str = "default";
// Context for our reconciler
#[derive(Clone)]
struct Context {
    /// Kubernetes client
    client: Client,
}

#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[kube(
    group = "letsgopherit.com",
    version = "v1",
    kind = "Blog",
    plural = "blogs",
    derive = "PartialEq",
    namespaced,
    status = "BlogStatus",
    shortname = "blog"
)]
pub struct BlogSpec {
    author: Vec<String>,
    title: String,
    draft: bool,
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

async fn reconcile(blog: Arc<Blog>, ctx: Arc<Context>) -> Result<Action> {
    let client = ctx.client.clone();
    let name = blog.name_any();
    let ns = blog.namespace().unwrap_or(DEFAULT_NS.to_string());

    let blogs: Api<Blog> = Api::namespaced(client.clone(), &ns);

    let action = finalizer(&blogs, BLOG_FINALIZER, blog, |event| async move {
        match event {
            Event::Apply(b) => b.reconcile(ctx).await,
            Event::Cleanup(b) => b.cleanup(ctx).await,
        }
    })
    .await
    .map_err(Error::FinalizerError);

    info!("Reconciled Blog \"{}\" in {}", name, ns);

    action
}

impl Blog {
    async fn reconcile(&self, ctx: Arc<Context>) -> Result<Action, kube::Error> {
        let client = ctx.client.clone();
        let name = self.name_any();
        let ns = self.namespace().unwrap();
        let blogs: Api<Blog> = Api::namespaced(client, &ns);

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

    async fn cleanup(&self, ctx: Arc<Context>) -> Result<Action, kube::Error> {
        info!("Cleaning up Blog \"{}\"", self.name_any());
        Ok(Action::await_change())
    }
}

pub struct Manager {}

impl Manager {
    pub async fn new() -> (
        Self,
        BoxFuture<'static, ()>, // This is a type alias for Pin<Box<dyn Future<Output = ()> + Send>>
    ) {
        let client = Client::try_default().await.unwrap();
        let blogs = Api::<Blog>::all(client.clone());
        let ctx = Arc::new(Context { client });
        let _l = blogs
            .list(&ListParams::default().limit(1))
            .await
            .expect("is the crd installed? please run: cargo run --bin crdgen | kubectl apply -f -");

        let controller = Controller::new(blogs, ListParams::default())
            .run(reconcile, error_policy, ctx)
            .filter_map(|x| async move { std::result::Result::ok(x) })
            .for_each(|_| futures::future::ready(()))
            .boxed();

        (Self {}, controller)
    }
}

fn error_policy(_blog: Arc<Blog>, error: &Error, _ctx: Arc<Context>) -> Action {
    warn!("reconcile failed: {:?}", error);
    Action::requeue(Duration::from_secs(5 * 60))
}
