use common_lib::types::v0::transport as v0;
use deployer_cluster::{result_either, test_result_grpc, Cluster, ClusterBuilder};
use grpc::operations::nexus::traits::NexusOperations;
use openapi::{apis::Uuid, models};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn create_nexus_malloc() {
    let cluster = ClusterBuilder::builder().build().await.unwrap();
    let nexus_client = cluster.grpc_client().nexus();

    nexus_client
        .create(
            &v0::CreateNexus {
                node: cluster.node(0),
                uuid: v0::NexusId::new(),
                size: 10 * 1024 * 1024,
                children: vec![
                    "malloc:///disk?size_mb=100&uuid=281b87d3-0401-459c-a594-60f76d0ce0da".into(),
                ],
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn create_nexus_sizes() {
    let cluster = ClusterBuilder::builder()
        .with_rest_timeout(std::time::Duration::from_secs(2))
        // don't log whilst we have the allow_fail
        .compose_build(|c| c.with_logs(false))
        .await
        .unwrap();
    let nexus_client = cluster.grpc_client().nexus();

    for size_mb in &vec![6, 10, 100] {
        let size = size_mb * 1024 * 1024;
        let disk = || {
            format!(
                "malloc:///disk?size_mb={}&uuid=281b87d3-0401-459c-a594-60f76d0ce0da",
                size_mb
            )
        };
        let sizes = vec![Ok(size / 2), Ok(size), Err(size + 512)];
        for test in sizes {
            let size = result_either!(test);
            test_result_grpc(&test, async {
                let nexus = nexus_client
                    .create(
                        &v0::CreateNexus {
                            node: cluster.node(0),
                            uuid: v0::NexusId::new(),
                            size,
                            children: vec![disk().into()],
                            ..Default::default()
                        },
                        None,
                    )
                    .await;

                if let Ok(nexus) = &nexus {
                    nexus_client
                        .destroy(&v0::DestroyNexus::from(nexus.clone()), None)
                        .await
                        .unwrap();
                }
                nexus
            })
            .await
            .unwrap();
        }
    }

    for size_mb in &vec![1, 2, 4] {
        let size = size_mb * 1024 * 1024;
        let disk = || {
            format!(
                "malloc:///disk?size_mb={}&uuid=281b87d3-0401-459c-a594-60f76d0ce0da",
                size_mb
            )
        };
        let sizes = vec![Err(size / 2), Err(size), Err(size + 512)];
        for test in sizes {
            let size = result_either!(test);
            test_result_grpc(&test, async {
                let nexus = nexus_client
                    .create(
                        &v0::CreateNexus {
                            node: cluster.node(0),
                            uuid: v0::NexusId::new(),
                            size,
                            children: vec![disk().into()],
                            ..Default::default()
                        },
                        None,
                    )
                    .await;
                if let Ok(nexus) = &nexus {
                    nexus_client
                        .destroy(&v0::DestroyNexus::from(nexus.clone()), None)
                        .await
                        .unwrap();
                }
                nexus
            })
            .await
            .unwrap();
        }
    }
}

#[tokio::test]
async fn create_nexus_local_replica() {
    let size = 10 * 1024 * 1024;
    let cluster = ClusterBuilder::builder()
        .with_pools(1)
        .with_replicas(1, size, v0::Protocol::None)
        .build()
        .await
        .unwrap();
    let nexus_client = cluster.grpc_client().nexus();

    let replica = cluster
        .rest_v00()
        .replicas_api()
        .get_replica(&Cluster::replica(0, 0, 0))
        .await
        .unwrap();

    nexus_client
        .create(
            &v0::CreateNexus {
                node: cluster.node(0),
                uuid: v0::NexusId::new(),
                size,
                children: vec![replica.uri.into()],
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn create_nexus_replicas() {
    let size = 10 * 1024 * 1024;
    let cluster = ClusterBuilder::builder()
        .with_io_engines(2)
        .with_pools(1)
        .with_replicas(1, size, v0::Protocol::None)
        .build()
        .await
        .unwrap();
    let nexus_client = cluster.grpc_client().nexus();
    let local = cluster
        .rest_v00()
        .replicas_api()
        .get_replica(&Cluster::replica(0, 0, 0))
        .await
        .unwrap();
    let remote = cluster
        .rest_v00()
        .replicas_api()
        .put_node_pool_replica_share(
            cluster.node(1).as_str(),
            cluster.pool(1, 0).as_str(),
            &(Cluster::replica(1, 0, 0).into()),
        )
        .await
        .unwrap();

    nexus_client
        .create(
            &v0::CreateNexus {
                node: cluster.node(0),
                uuid: v0::NexusId::new(),
                size,
                children: vec![local.uri.into(), remote.into()],
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn create_nexus_replica_not_available() {
    let size = 10 * 1024 * 1024;
    let cluster = ClusterBuilder::builder()
        .with_rest(true)
        .with_agents(vec!["core"])
        .with_io_engines(2)
        .with_pools(1)
        .with_replicas(1, size, v0::Protocol::None)
        .build()
        .await
        .unwrap();

    let local = cluster
        .rest_v00()
        .replicas_api()
        .get_replica(&Cluster::replica(0, 0, 0))
        .await
        .unwrap();
    let remote = cluster
        .rest_v00()
        .replicas_api()
        .put_pool_replica_share(cluster.pool(1, 0).as_str(), &Cluster::replica(1, 0, 0))
        .await
        .unwrap();
    cluster
        .rest_v00()
        .replicas_api()
        .del_pool_replica_share(cluster.pool(1, 0).as_str(), &Cluster::replica(1, 0, 0))
        .await
        .unwrap();
    cluster
        .rest_v00()
        .nexuses_api()
        .put_node_nexus(
            cluster.node(0).as_str(),
            &v0::NexusId::new(),
            models::CreateNexusBody::new(vec![local.uri, remote], size),
        )
        .await
        .expect_err("One replica is not present so nexus shouldn't be created");
}

#[tokio::test]
async fn kato() {
    let cluster = ClusterBuilder::builder()
        .with_rest(true)
        .with_agents(vec!["core"])
        .with_io_engines(2)
        .with_options(|o| {
            o.with_isolated_io_engine(true)
                .with_io_engine_env("NVME_KATO_MS", "10000")
                .with_io_engine_env("NVME_TIMEOUT_US", 200_000.to_string().as_str())
                .with_io_engine_env("NVME_TIMEOUT_ADMIN_US", 10_000_000.to_string().as_str())
        })
        .with_pools(1)
        .with_cache_period("500ms")
        .build()
        .await
        .unwrap();

    let api = cluster.rest_v00();
    let node_id = cluster.node(0);
    let volumes_api = api.volumes_api();
    let volume = volumes_api
        .put_volume(
            &Uuid::new_v4(),
            models::CreateVolumeBody::new(models::VolumePolicy::new(false), 2, 5242880u64, true),
        )
        .await
        .unwrap();

    let volume = volumes_api
        .put_volume_target(
            &volume.spec.uuid,
            models::VolumeShareProtocol::Nvmf,
            Some(node_id.as_str()),
        )
        .await
        .unwrap();

    let mut rpc = cluster.grpc_handle(cluster.node(0).as_str()).await.unwrap();
    cluster
        .composer()
        .pause(cluster.node(1).as_str())
        .await
        .unwrap();

    let start = std::time::Instant::now();
    loop {
        let nexuses = rpc
            .io_engine
            .list_nexus(rpc::io_engine::Null {})
            .await
            .unwrap()
            .into_inner();
        let nexus = nexuses.nexus_list.first().unwrap();
        assert_eq!(nexus.children.len(), 2);
        if nexus.state != rpc::io_engine::NexusState::NexusOnline as i32 {
            println!("Took {:?} to fault", start.elapsed());
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    volumes_api.del_volume(&volume.spec.uuid).await.unwrap();
}

#[tokio::test]
async fn hammer() {
    let cluster = ClusterBuilder::builder()
        .with_rest(true)
        .with_agents(vec!["core"])
        .with_io_engines(3)
        .with_options(|o| {
            o.with_isolated_io_engine(true)
                .with_io_engine_devices(vec![
                    "/dev/sdavg/lvol0",
                    "/dev/sdavg/lvol1",
                    "/dev/sdavg/lvol2",
                ])
                .with_io_engine_env("NVME_KATO_MS", "100")
                .with_io_engine_env("NVME_TIMEOUT_US", 200_000.to_string().as_str())
        })
        .with_pool(0, "/dev/sdavg/lvol0")
        .with_pool(1, "/dev/sdavg/lvol1")
        .with_pool(2, "/dev/sdavg/lvol2")
        .with_cache_period("500ms")
        .build()
        .await
        .unwrap();

    let mut tasks = vec![];

    for node in 0 .. 3 {
        let api = cluster.rest_v00();
        let node_id = cluster.node(node);
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        let handle = tokio::spawn(async move {
            let volumes_api = api.volumes_api();

            for _ in 0 .. 1000 {
                let volumes = Arc::new(Mutex::new(Vec::with_capacity(100)));
                let mut handles = vec![];
                for _ in 0 .. 10 {
                    let api = api.clone();
                    let node_id = node_id.clone();
                    let volumes = volumes.clone();
                    let handle = tokio::spawn(async move {
                        let volumes_api = api.volumes_api();
                        for _ in 0 .. 10 {
                            let volume = volumes_api
                                .put_volume(
                                    &Uuid::new_v4(),
                                    models::CreateVolumeBody::new(
                                        models::VolumePolicy::new(false),
                                        3,
                                        5242880u64,
                                        true,
                                    ),
                                )
                                .await
                                .unwrap();

                            let volume = volumes_api
                                .put_volume_target(
                                    &volume.spec.uuid,
                                    models::VolumeShareProtocol::Nvmf,
                                    Some(node_id.as_str()),
                                )
                                .await
                                .unwrap();
                            volumes.lock().unwrap().push(volume);
                        }
                    });
                    handles.push(handle);
                }

                futures::future::try_join_all(handles).await.unwrap();

                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                let volumes = volumes.lock().unwrap().clone();
                for volume in volumes {
                    volumes_api
                        .del_volume_target(&volume.spec.uuid, Some(false))
                        .await
                        .unwrap();
                    volumes_api.del_volume(&volume.spec.uuid).await.unwrap();
                }
            }
        });
        tasks.push(handle);
    }

    futures::future::try_join_all(tasks).await.unwrap();
}
