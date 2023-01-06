// Copyright 2022 The Amphitheatre Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;
use std::time::Duration;

use k8s_openapi::apiextensions_apiserver as server;
use kube::api::{DeleteParams, Patch, PatchParams, PostParams};
use kube::core::{DynamicObject, GroupVersionKind};
use kube::discovery::ApiResource;
use kube::{Api, Client, CustomResourceExt, ResourceExt};
use serde_json::{from_value, json, to_string_pretty};
use server::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use tokio::time::sleep;

use super::error::{Error, Result};
use super::types::{Actor, Playbook, PlaybookSpec, PLAYBOOK_RESOURCE_NAME};
use crate::composer::types::PlaybookStatus;

pub async fn install(client: Client) -> Result<()> {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    let crd = Playbook::crd();
    tracing::debug!(
        "Creating the Playbook CustomResourceDefinition: {}",
        to_string_pretty(&crd).unwrap()
    );

    let params = PostParams::default();
    match api.create(&params, &crd).await {
        Ok(o) => {
            tracing::debug!("Created {} ({:?})", o.name_any(), o.status.unwrap());
            tracing::debug!("Created CRD: {:?}", o.spec);
        }
        Err(kube::Error::Api(err)) => assert_eq!(err.code, 409), /* if you skipped delete, for instance */
        Err(err) => return Err(Error::KubeError(err)),
    }

    // Wait for the api to catch up
    sleep(Duration::from_secs(1)).await;

    Ok(())
}

pub async fn uninstall(client: Client) -> Result<()> {
    let api: Api<CustomResourceDefinition> = Api::all(client);
    let params = DeleteParams::default();

    // Ignore delete error if not exists
    let _ = api
        .delete(PLAYBOOK_RESOURCE_NAME, &params)
        .await
        .map_err(Error::KubeError)?
        .map_left(|o| tracing::debug!("Deleting CRD: {:?}", o.status))
        .map_right(|s| tracing::debug!("Deleted CRD: {:?}", s));

    // Wait for the delete to take place (map-left case or delete from previous run)
    sleep(Duration::from_secs(2)).await;

    Ok(())
}

pub async fn create(
    client: Client,
    namespace: String,
    name: String,
    title: String,
    description: String,
) -> Result<Playbook> {
    let api: Api<Playbook> = Api::namespaced(client.clone(), namespace.as_str());
    let params = PostParams::default();

    let mut playbook = Playbook::new(
        name.as_str(),
        PlaybookSpec {
            title,
            description,
            actors: vec![Actor {
                name: "amp-example-java".into(),
                description: "A simple Java example app".into(),
                image: "amp-example-java".into(),
                repo: "https://github.com/amphitheatre-app/amp-example-java".into(),
                path: ".".into(),
                reference: "master".into(),
                commit: "875db185acc8bf7c7effc389a350cae7aa926e57".into(),
                environment: HashMap::new(),
                partners: vec![
                    "https://github.com/amphitheatre-app/amp-example-nodejs.git".to_string()
                ],
            }],
        },
    );

    tracing::info!("{:#?}", serde_yaml::to_string(&playbook));
    playbook = api
        .create(&params, &playbook)
        .await
        .map_err(Error::KubeError)?;

    // Patch this playbook as initial Pending status
    status(client.clone(), &playbook, PlaybookStatus::Pending).await?;
    Ok(playbook)
}

pub async fn status(client: Client, playbook: &Playbook, status: PlaybookStatus) -> Result<()> {
    let namespace = playbook
        .namespace()
        .ok_or_else(|| Error::MissingObjectKey(".metadata.namespace"))?;
    let api: Api<Playbook> = Api::namespaced(client, namespace.as_str());

    let status = json!({ "status": status });
    let playbook = api
        .patch_status(
            playbook.name_any().as_str(),
            &PatchParams::default(),
            &Patch::Merge(&status),
        )
        .await
        .map_err(Error::KubeError)?;

    tracing::info!(
        "Patched status {:?} for {}",
        playbook.status,
        playbook.name_any()
    );

    Ok(())
}

pub async fn build(client: Client, playbook: &Playbook, actor: &Actor) -> Result<()> {
    let namespace = playbook
        .namespace()
        .ok_or_else(|| Error::MissingObjectKey(".metadata.namespace"))?;

    let gvk = GroupVersionKind::gvk("kpack.io", "v1alpha2", "Image");
    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(client, namespace.as_str(), &ar);

    let params = PostParams::default();
    let resource = from_value(json!({
        "apiVersion": "kpack.io/v1alpha2",
        "kind": "Image",
        "metadata": {
            "name": format!("{}-{}", actor.name, actor.commit),
        },
        "spec": {
            "tag": format!("harbor.amp-system.svc.cluster.local/library/{}:{}", actor.image, actor.commit),
            "serviceAccountName": "default",
            "builder": {
                "name": "amp-default-cluster-builder",
                "kind": "ClusterBuilder",
            },
            "source": {
                "git": {
                    "url": actor.repo,
                    "revision": actor.commit,
                },
                "subPath": actor.path,
            }
        }
    }))
    .map_err(Error::SerializationError)?;

    tracing::info!(
        "created image resource: {:#?}",
        serde_yaml::to_string(&resource)
    );
    api.create(&params, &resource)
        .await
        .map_err(Error::KubeError)?;

    Ok(())
}

pub async fn add(client: Client, playbook: &Playbook, actor: Actor) -> Result<()> {
    let namespace = playbook
        .namespace()
        .ok_or_else(|| Error::MissingObjectKey(".metadata.namespace"))?;
    let api: Api<Playbook> = Api::namespaced(client, namespace.as_str());

    let actor_name = actor.name.clone();
    let mut actors = playbook.spec.actors.clone();
    actors.push(actor);

    let patch = json!({"spec": { "actors": actors }});
    let playbook = api
        .patch(
            playbook.name_any().as_str(),
            &PatchParams::apply("amp-composer"),
            &Patch::Merge(&patch),
        )
        .await
        .map_err(Error::KubeError)?;

    tracing::info!("Added actor {:?} for {}", actor_name, playbook.name_any());

    Ok(())
}
