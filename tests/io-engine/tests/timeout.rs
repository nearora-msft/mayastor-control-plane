use deployer_cluster::{Cluster, ClusterBuilder, FindVolumeRequest};
use openapi::{apis::Uuid, models};
use rpc::io_engine::IoEngineApiVersion;
use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;

#[tokio::test]
async fn timeout() {
    let nr_volumes = 2u32;

    let replica_count = 3;
    let io_engines = 4u32;
    let gig = 1024 * 1024 * 1024u64;
    let pool_size_bytes = (nr_volumes as u64) * (replica_count as u64 + 1) * gig;

    let cluster = ClusterBuilder::builder()
        .with_options(|o| {
            o.with_io_engines(io_engines as u32)
                .with_csi(false, true)
                .with_isolated_io_engine(false)
                .with_io_engine_env("NVME_QPAIR_CONNECT_ASYNC", "false")
                .with_io_engine_api(vec![IoEngineApiVersion::V0])
                .with_fio_spdk(true)
        })
        .with_rest_timeout(Duration::from_secs(30))
        //.with_cache_period("1s")
        .with_tmpfs_pool(pool_size_bytes)
        .compose_build(|b| b.with_logs(false).with_clean(false))
        .await
        .unwrap();

    let cluster = Arc::new(cluster);
    // {
    //     let cluster = cluster.clone();
    //     tokio::spawn(async move {
    //         run_fio_forever(&cluster, nr_volumes, replica_count, io_engines).await;
    //     });
    // }

    let mut tasks = vec![];

    // fio on host
    for i in 0 .. nr_volumes {
        let cluster = cluster.clone();
        tokio::time::sleep(Duration::from_secs(1)).await;
        let h = tokio::spawn(async move {
            loop {
                let volume = create_volume(&cluster, 3, i % io_engines).await;

                // fio as well?
                let tasks = run_fio_vols(&cluster, vec![volume.clone()], false).await;
                for task in tasks {
                    task.await.unwrap();
                }

                destroy_volumes(&cluster, vec![volume]).await;
            }
        });
        tasks.push(h);
    }

    for task in tasks {
        check_volumes(&cluster).await;
        task.await.unwrap();
    }

    loop {
        // create x volumes and run fio on them
        let mut tasks = vec![];

        // userspace fio
        for i in 0 .. nr_volumes / 2 {
            let cluster = cluster.clone();
            let h = tokio::spawn(async move {
                let volume = create_volume(&cluster, replica_count, i % io_engines).await;

                let tasks = run_fio_spdk_vols(&cluster, vec![volume.clone()], false).await;
                for task in tasks {
                    task.await.unwrap();
                }

                destroy_volumes(&cluster, vec![volume]).await;
            });
            tasks.push(h);
        }
        // fio on host
        for i in 0 .. nr_volumes / 2 {
            let cluster = cluster.clone();
            let h = tokio::spawn(async move {
                let volume = create_volume(&cluster, 3, i % io_engines).await;

                // fio as well?
                let tasks = run_fio_vols(&cluster, vec![volume.clone()], false).await;
                for task in tasks {
                    task.await.unwrap();
                }

                destroy_volumes(&cluster, vec![volume]).await;
            });
            tasks.push(h);
        }

        {
            for i in 0 .. nr_volumes / 2 {
                let cluster = cluster.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(5)).await;

                    let volumes =
                        create_volumes(&cluster, 10, replica_count, (0, io_engines)).await;
                    destroy_volumes(&cluster, volumes).await;
                });
            }
        }

        for task in tasks {
            check_volumes(&cluster).await;
            task.await.unwrap();
        }

        check_volumes(&cluster).await;
    }

    // make sure we clean up...
    cluster
        .composer()
        .exec(
            cluster.csi_container(0).as_str(),
            vec!["nvme", "disconnect-all"],
        )
        .await
        .unwrap();
}

async fn create_volumes(
    cluster: &Cluster,
    count: u32,
    repl: u8,
    node: (u32, u32),
) -> Vec<models::Volume> {
    let api = cluster.rest_v00();
    let vol_cli = api.volumes_api();
    let mut volumes = Vec::with_capacity(count as usize);

    let mut nodeplace = node.0;
    for _ in 0 .. count {
        let volume = vol_cli
            .put_volume(
                &Uuid::new_v4(),
                models::CreateVolumeBody::new(
                    models::VolumePolicy::new(false),
                    repl,
                    50241024u64,
                    false,
                ),
            )
            .await
            .unwrap();
        match vol_cli
            .put_volume_target(
                &volume.spec.uuid,
                models::VolumeShareProtocol::Nvmf,
                Some(cluster.node(nodeplace).as_str()),
                Some(false),
            )
            .await
        {
            Ok(volume) => volumes.push(volume),
            Err(error) => tracing::error!("ERR: {:?}", error),
        }

        if node.1 != 0 {
            nodeplace = (nodeplace + 1) % node.1
        }
    }
    volumes
}
async fn destroy_volumes(cluster: &Cluster, volumes: Vec<models::Volume>) {
    let api = cluster.rest_v00();
    let api = api.volumes_api();
    for volume in volumes {
        api.del_volume(&volume.spec.uuid).await.unwrap();
    }
}

async fn run_fio_forever(cluster: &Arc<Cluster>, count: u32, repl: u8, nodes: u32) {
    let volumes = create_volumes(&cluster, count, repl, (0, nodes)).await;

    let tasks = run_fio_spdk_vols(cluster, volumes, true).await;

    for task in tasks {
        task.await.unwrap();
    }

    unreachable!("Diamonds are forever!");
}

async fn run_fio_spdk_vols(
    cluster: &Arc<Cluster>,
    volumes: Vec<models::Volume>,
    forever: bool,
) -> Vec<JoinHandle<()>> {
    let fio_builder = |ip: &str, uuid: &str| {
        let nqn = format!("nqn.2019-05.io.openebs\\:{uuid}");
        let filename =
            format!("--filename=trtype=tcp adrfam=IPv4 traddr={ip} trsvcid=8420 subnqn={nqn} ns=1");
        tracing::info!("F: {}", filename);
        vec![
            "fio",
            "--name=benchtest",
            filename.as_str(),
            "--direct=1",
            "--rw=randrw",
            "--ioengine=spdk",
            "--bs=4k",
            "--iodepth=32",
            "--numjobs=1",
            "--thread=1",
            "--size=20M",
            "--time_based",
            "--runtime=120",
            "--norandommap=1",
        ]
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
    };

    let mut tasks = vec![];
    for volume in volumes {
        let node = volume.state.target.as_ref().unwrap().node.clone();
        let ip = cluster.composer().container_ip(node.as_str());
        let fio_cmd = fio_builder(&ip, &volume.spec.uuid.to_string());

        let composer = cluster.composer().clone();
        let forever = forever;
        let cluster = cluster.clone();
        let handle = tokio::spawn(async move {
            loop {
                let mut node = cluster.csi_node_client(0).await.unwrap();
                node.node_stage_volume_fs(&volume).await.unwrap();

                let (code, out) = composer.exec("fio-spdk", fio_cmd.clone()).await.unwrap();
                tracing::info!(
                    "{:?}: {}",
                    fio_cmd.iter().map(|e| format!("{e} ")).collect::<String>(),
                    out
                );
                if code != Some(0) {
                    tracing::info!(
                        "{:?}: {}",
                        fio_cmd
                            .into_iter()
                            .map(|e| format!("{e} "))
                            .collect::<String>(),
                        out
                    );
                    std::process::exit(1);
                }
                assert_eq!(code, Some(0));

                node.node_unstage_volume(&volume).await.unwrap();

                if !forever {
                    break;
                }
            }
        });
        tasks.push(handle);
    }
    tasks
}

async fn run_fio_vols(
    cluster: &Cluster,
    volumes: Vec<models::Volume>,
    forever: bool,
) -> Vec<JoinHandle<()>> {
    let fio_builder = |device: &str| {
        let filename = format!("--filename={device}");
        vec![
            "fio",
            "--name=benchtest",
            filename.as_str(),
            // "--direct=1",
            "--ioengine=aio",
            "--rw=randrw",
            "--bs=4k",
            "--iodepth=32",
            "--numjobs=1",
            "--thread=1",
            "--time_based",
            "--runtime=120",
            "--size=20M",
            "--norandommap=1",
        ]
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
    };

    let mut tasks = vec![];
    for volume in volumes {
        let mut node = cluster.csi_node_client(0).await.unwrap();
        node.node_stage_volume_fs(&volume).await.unwrap();

        // let response = node
        //     .internal()
        //     .find_volume(FindVolumeRequest {
        //         volume_id: volume.spec.uuid.to_string(),
        //     })
        //     .await
        //     .unwrap();
        //
        // let device_path = response.into_inner().device_path;

        let path = format!(
            "/var/tmp/staging/mount/{}/file.io",
            volume.spec.uuid.to_string()
        );

        let fio_cmd = fio_builder(&path);
        let composer = cluster.composer().clone();

        let handle = tokio::spawn(async move {
            loop {
                let (code, out) = composer.exec("fio-spdk", fio_cmd.clone()).await.unwrap();
                tracing::info!(
                    "{:?}: {}",
                    fio_cmd.iter().map(|e| format!("{e} ")).collect::<String>(),
                    out
                );
                if code != Some(0) {
                    tracing::info!(
                        "{:?}: {}",
                        fio_cmd
                            .into_iter()
                            .map(|e| format!("{e} "))
                            .collect::<String>(),
                        out
                    );
                    std::process::exit(1);
                }
                assert_eq!(code, Some(0));

                if !forever {
                    break;
                }
            }

            node.node_unstage_volume(&volume).await.unwrap();
        });
        tasks.push(handle);
    }
    tasks
}

async fn create_volume(cluster: &Cluster, repl: u8, node: u32) -> models::Volume {
    let api = cluster.rest_v00();
    let vol_cli = api.volumes_api();

    let volume = vol_cli
        .put_volume(
            &Uuid::new_v4(),
            models::CreateVolumeBody::new(
                models::VolumePolicy::new(false),
                repl,
                50241024u64,
                true,
            ),
        )
        .await
        .unwrap();
    vol_cli
        .put_volume_target(
            &volume.spec.uuid,
            models::VolumeShareProtocol::Nvmf,
            Some(cluster.node(node).as_str()),
            Some(false),
        )
        .await
        .unwrap()
}

async fn check_volumes(cluster: &Cluster) {
    let api = cluster.rest_v00();
    let vol_cli = api.volumes_api();
    let volumes = vol_cli.get_volumes(0, None).await.unwrap().entries;
    // volumes should either be online or degraded (while rebuilding)
    let not_expected = volumes
        .iter()
        .filter(|v| !matches!(v.state.status, models::VolumeStatus::Online))
        .collect::<Vec<_>>();
    assert!(not_expected.is_empty());
}
