use crate::infra::{
    async_trait, Builder, ComponentAction, ComposeTest, Error, FioSpdk, StartOptions,
};
use composer::ContainerSpec;

#[async_trait]
impl ComponentAction for FioSpdk {
    fn configure(&self, options: &StartOptions, cfg: Builder) -> Result<Builder, Error> {
        Ok(if options.fio_spdk {
            cfg.add_container_spec(
                ContainerSpec::from_image("fio-spdk", utils::FIO_SPDK_IMAGE)
                    .with_entrypoint("sleep")
                    .with_bypass_default_mounts(true)
                    .with_bind("/var/run/dpdk", "/var/run/dpdk")
                    .with_bind("/var/tmp/", "/var/tmp/:shared")
                    .with_bind("/dev", "/dev:ro")
                    .with_privileged(Some(true))
                    .with_arg("infinity"),
            )
        } else {
            cfg
        })
    }
    async fn start(&self, options: &StartOptions, cfg: &ComposeTest) -> Result<(), Error> {
        if options.fio_spdk {
            cfg.start("fio-spdk").await?;
        }
        Ok(())
    }
    async fn wait_on(&self, _options: &StartOptions, _cfg: &ComposeTest) -> Result<(), Error> {
        // this is fine 🔥
        Ok(())
    }
}
