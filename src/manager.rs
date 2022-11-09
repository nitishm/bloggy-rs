use std::sync::Arc;
use std::time::Duration;

use crate::api::*;
use crate::{Context, Error, Result};
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use kube::api::ListParams;
use kube::runtime::finalizer::{finalizer, Event};
use kube::{
    runtime::controller::{Action, Controller},
    Api, Client, ResourceExt,
};
use tracing::*;

static BLOG_FINALIZER: &str = "blogs.letsgopherit.com";

async fn reconcile(blog: Arc<Blog>, ctx: Arc<Context>) -> Result<Action> {
    let client = ctx.client.clone();
    let name = blog.name_any();

    let blogs: Api<Blog> = Api::all(client.clone());

    let action = finalizer(&blogs, BLOG_FINALIZER, blog, |event| async move {
        match event {
            Event::Apply(b) => b.reconcile(ctx).await,
            Event::Cleanup(b) => b.cleanup(ctx).await,
        }
    })
    .await
    .map_err(Error::FinalizerError);

    info!("Reconciled Blog \"{}\"", name);

    action
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

fn error_policy(blog: Arc<Blog>, error: &Error, _ctx: Arc<Context>) -> Action {
    warn!("reconcile failed for blog {}: {:?}", blog.name_any(), error);
    Action::requeue(Duration::from_secs(5 * 60))
}
