"""Cordoning feature tests."""
import time

from pytest_bdd import (
    given,
    scenario,
    then,
    when,
)

from retrying import retry
import pytest
import os
import requests

from common.deployer import Deployer
from common.apiclient import ApiClient
from common.docker import Docker

from openapi.model.create_pool_body import CreatePoolBody
from openapi.model.create_replica_body import CreateReplicaBody
from openapi.model.create_nexus_body import CreateNexusBody
from openapi.model.create_volume_body import CreateVolumeBody
from openapi.model.volume_policy import VolumePolicy
from openapi.model.protocol import Protocol
from openapi.model.volume_status import VolumeStatus


NODE_1_NAME = "io-engine-1"
VOLUME_1_UUID = "5cd5378e-3f05-47f1-a830-a0f5873a1449"
NEXUS_1_UUID = "9a024ca4-b793-490c-b65c-cbeb176671e5"
REPLICA_1_UUID = "5ca0f4b2-5e77-401e-bdc5-647fd3feb35f"
POOL_DISK1 = "pdisk1.img"
POOL_1_UUID = "4cc6ee64-7232-497d-a26f-38284a444980"

NODE_2_NAME = "io-engine-2"
VOLUME_2_UUID = "30dae813-3cb6-4833-86b4-364f1eed213e"
NEXUS_2_UUID = "54ff9b09-9399-422d-b7a0-a4c838efe840"
REPLICA_2_UUID = "cbbf19de-f107-4f41-a368-1f05b54cd4b1"
POOL_DISK2 = "pdisk2.img"
POOL_2_UUID = "5df23272-33aa-45ad-a2c0-560006bc87e4"

VOLUME_SIZE = 10485761
CORDON_LABEL_1 = "test-cordon-label"
CORDON_LABEL_2 = "test-cordon-label-2"
NODE_CORDONED_RESOURCE_KEY = "node_cordoned_resource"
NODE_CORDON_ERROR_KEY = "node_cordon_error"
NODE_UNCORDONED_RESOURCE_KEY = "node_uncordoned_resource"
NODE_UNCORDON_ERROR_KEY = "node_uncordon_error"


@pytest.fixture(scope="function")
def create_pool_disk_images():
    # When starting Io-Engine instances with the deployer a bind mount is created from /tmp to
    # /host/tmp, so create disk images in /tmp
    for disk in [POOL_DISK1, POOL_DISK2]:
        path = "/tmp/{}".format(disk)
        with open(path, "w") as file:
            file.truncate(100 * 1024 * 1024)

    yield
    for disk in [POOL_DISK1, POOL_DISK2]:
        path = "/tmp/{}".format(disk)
        if os.path.exists(path):
            os.remove(path)


@pytest.fixture(autouse=True)
def init(create_pool_disk_images):
    Deployer.start(2, reconcile_period="1s", cache_period="1s")
    # Create various resources on one of the nodes
    create_node_resources(
        NODE_1_NAME, POOL_1_UUID, REPLICA_1_UUID, NEXUS_1_UUID, VOLUME_1_UUID
    )
    yield
    Deployer.stop()


# Fixture used to pass the cordoned resources between test steps.
@pytest.fixture(scope="function")
def cordoned_resources():
    return {}


# Fixture used to pass the uncordoned resources between test steps.
@pytest.fixture(scope="function")
def uncordoned_resources():
    return {}


# Fixture used to pass the cordon error between test steps.
@pytest.fixture(scope="function")
def cordon_error():
    return {}


# Fixture used to pass the uncordon error between test steps.
@pytest.fixture(scope="function")
def uncordon_error():
    return {}


@scenario("feature.feature", "Cordoning a cordoned node")
def test_cordoning_a_cordoned_node():
    """Cordoning a cordoned node."""


@scenario("feature.feature", "Cordoning a node")
def test_cordoning_a_node():
    """Cordoning a node."""


@scenario("feature.feature", "Cordoning a node with an existing label")
def test_cordoning_a_node_with_an_existing_label():
    """Cordoning a node with an existing label."""


@scenario("feature.feature", "Cordoning a node with existing resources")
def test_cordoning_a_node_with_existing_resources():
    """Cordoning a node with existing resources."""


@scenario("feature.feature", "Deleting resources on a cordoned node")
def test_deleting_resources_on_a_cordoned_node():
    """Deleting resources on a cordoned node."""


@scenario("feature.feature", "Restarting a cordoned node")
def test_restarting_a_cordoned_node():
    """Restarting a cordoned node."""


@scenario("feature.feature", "Uncordoning a node")
def test_uncordoning_a_node():
    """Uncordoning a node."""


@scenario("feature.feature", "Uncordoning a node with an unknown label")
def test_uncordoning_a_node_with_an_unknown_label():
    """Uncordoning a node with an unknown label."""


@scenario("feature.feature", "Uncordoning a node with existing resources")
def test_uncordoning_a_node_with_existing_resources():
    """Uncordoning a node with existing resources."""


@scenario("feature.feature", "Uncordoning an uncordoned node")
def test_uncordoning_an_uncordoned_node():
    """Uncordoning an uncordoned node."""


@scenario("feature.feature", "Unschedulable replicas due to node cordon")
def test_unschedulable_replicas_due_to_node_cordon():
    """Unschedulable replicas due to node cordon."""


@given("Multiple uncordoned nodes")
def multiple_uncordoned_nodes():
    """Multiple uncordoned nodes."""
    assert len(get_uncordoned_nodes()) > 1


@given("a cordoned node")
def a_cordoned_node(cordoned_resources):
    """a cordoned node."""
    cordoned_node = cordon_node(NODE_1_NAME, CORDON_LABEL_1)
    assert len(get_cordoned_nodes()) > 0
    cordoned_resources[NODE_CORDONED_RESOURCE_KEY] = cordoned_node


@given("a cordoned node with resources")
def a_cordoned_node_with_resources(cordoned_resources):
    """a cordoned node with resources."""
    # Cordon the node with resources on it
    cordoned_node = cordon_node(NODE_1_NAME, CORDON_LABEL_1)
    cordoned_resources[NODE_CORDONED_RESOURCE_KEY] = cordoned_node


@given("a published volume with multiple replicas")
def a_published_volume_with_multiple_replicas():
    """a published volume with multiple replicas."""
    # Before creating the volume create a second pool for the replica
    ApiClient.pools_api().put_node_pool(
        NODE_2_NAME,
        POOL_2_UUID,
        CreatePoolBody(["aio:///host/tmp/{}".format(POOL_DISK2)]),
    )
    ApiClient.volumes_api().put_volume(
        VOLUME_2_UUID, CreateVolumeBody(VolumePolicy(True), 2, VOLUME_SIZE)
    )
    ApiClient.volumes_api().put_volume_target(
        VOLUME_2_UUID, NODE_1_NAME, Protocol("nvmf")
    )


@given("an uncordoned node")
def an_uncordoned_node():
    """an uncordoned node."""
    assert len(get_uncordoned_nodes()) > 0


@given("an uncordoned node with resources")
def an_uncordoned_node_with_resources():
    """an uncordoned node with resources."""
    uncordoned_nodes = get_uncordoned_nodes()
    node = list(filter(lambda node: node["id"] == NODE_1_NAME, uncordoned_nodes))
    assert len(node) == 1

    # Check the resources exist on the node
    check_node_resources(
        node[0]["id"], POOL_1_UUID, REPLICA_1_UUID, NEXUS_1_UUID, VOLUME_1_UUID, False
    )


@when("the control plane attempts to delete resources on a cordoned node")
def the_control_plane_attempts_to_delete_resources_on_a_cordoned_node(
    cordoned_resources,
):
    """the control plane attempts to delete resources on a cordoned node."""
    cordoned_node_id = get_node_id(cordoned_resources[NODE_CORDONED_RESOURCE_KEY])
    # Delete all the resources on the cordoned node
    ApiClient.volumes_api().del_volume(VOLUME_1_UUID)
    ApiClient.nexuses_api().del_node_nexus(cordoned_node_id, NEXUS_1_UUID)
    ApiClient.replicas_api().del_node_pool_replica(
        cordoned_node_id, POOL_1_UUID, REPLICA_1_UUID
    )
    ApiClient.pools_api().del_node_pool(cordoned_node_id, POOL_1_UUID)


@when("the cordoned node is cordoned")
def the_cordoned_node_is_cordoned(cordoned_resources):
    """the cordoned node is cordoned."""
    cordoned_node_id = get_node_id(cordoned_resources[NODE_CORDONED_RESOURCE_KEY])
    # Cordon the node again
    cordon_node(cordoned_node_id, CORDON_LABEL_2)


@when("the cordoned node is restarted")
def the_cordoned_node_is_restarted(cordoned_resources):
    """the cordoned node is restarted."""
    cordoned_node_id = get_node_id(cordoned_resources[NODE_CORDONED_RESOURCE_KEY])
    Docker.restart_container(cordoned_node_id)


@when("the node is cordoned")
def the_node_is_cordoned(cordoned_resources):
    """the node is cordoned."""
    node = cordon_node(NODE_1_NAME, CORDON_LABEL_1)
    cordoned_resources[NODE_CORDONED_RESOURCE_KEY] = node


@when("the node is cordoned again with the same label")
def the_node_is_cordoned_again_with_the_same_label(cordoned_resources, cordon_error):
    """the node is cordoned again with the same label."""
    cordoned_node_id = get_node_id(cordoned_resources[NODE_CORDONED_RESOURCE_KEY])
    try:
        cordon_node(cordoned_node_id, CORDON_LABEL_1)
    except Exception as e:
        cordon_error[NODE_CORDON_ERROR_KEY] = e


@when("the node is uncordoned")
def the_node_is_uncordoned(uncordoned_resources):
    """the node is uncordoned."""
    uncordoned_node = ApiClient.nodes_api().delete_node_cordon(
        NODE_1_NAME, CORDON_LABEL_1
    )
    assert len(get_cordoned_nodes()) == 0
    uncordoned_resources[NODE_UNCORDONED_RESOURCE_KEY] = uncordoned_node


@when("the node is uncordoned using an unknown cordon label")
def the_node_is_uncordoned_using_an_unknown_cordon_label(
    cordoned_resources, uncordon_error
):
    """the node is uncordoned using an unknown cordon label."""
    cordoned_node_id = get_node_id(cordoned_resources[NODE_CORDONED_RESOURCE_KEY])
    try:
        ApiClient.nodes_api().delete_node_cordon(cordoned_node_id, "INVALID_LABEL")
    except Exception as e:
        uncordon_error[NODE_UNCORDON_ERROR_KEY] = e


@when("the uncordoned node is uncordoned")
def the_uncordoned_node_is_uncordoned(uncordoned_resources):
    """the uncordoned node is uncordoned."""
    assert len(get_cordoned_nodes()) == 0

    try:
        ApiClient.nodes_api().delete_node_cordon(NODE_1_NAME, CORDON_LABEL_1)
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["precondition_failed"]

    assert len(get_cordoned_nodes()) == 0
    uncordoned_resources[NODE_UNCORDONED_RESOURCE_KEY] = ApiClient.nodes_api().get_node(
        NODE_1_NAME
    )


@when("the volume becomes degraded")
def the_volume_becomes_degraded():
    """the volume becomes degraded."""
    Docker.kill_container(NODE_2_NAME)
    wait_degraded_volume(VOLUME_2_UUID)


@when("there are insufficient uncordoned nodes to accommodate new replicas")
def there_are_insufficient_uncordoned_nodes_to_accommodate_new_replicas():
    """there are insufficient uncordoned nodes to accommodate new replicas."""
    volume = ApiClient.volumes_api().get_volume(VOLUME_2_UUID)
    # The number of uncordoned nodes must be greater than the replica count if a volume is to recover when degraded.
    assert len(get_uncordoned_nodes()) <= volume["spec"]["num_replicas"]


@then("cordoning should fail")
def cordoning_should_fail(cordon_error):
    """cordoning should fail."""
    error = cordon_error[NODE_CORDON_ERROR_KEY]
    exception_info = error.__dict__
    assert exception_info["status"] == requests.codes["precondition_failed"]


@then("uncordoning should fail")
def uncordoning_should_fail(uncordon_error):
    """uncordoning should fail."""
    error = uncordon_error[NODE_UNCORDON_ERROR_KEY]
    exception_info = error.__dict__
    assert exception_info["status"] == requests.codes["precondition_failed"]


@then("existing resources remain unaffected")
def existing_resources_remain_unaffected(cordoned_resources):
    """existing resources remain unaffected."""
    cordoned_node_id = get_node_id(cordoned_resources[NODE_CORDONED_RESOURCE_KEY])

    # Check the pool is still on the cordoned node
    pool = ApiClient.pools_api().get_pool(POOL_1_UUID)
    assert pool["spec"]["node"] == cordoned_node_id

    # Check the replica is still on the cordoned node
    replica = ApiClient.replicas_api().get_replica(REPLICA_1_UUID)
    assert replica["node"] == cordoned_node_id

    # Check the nexus is still on the cordoned node
    nexus = ApiClient.nexuses_api().get_nexus(NEXUS_1_UUID)
    assert nexus["node"] == cordoned_node_id

    # Check the volume target is still on the cordoned node
    volume = ApiClient.volumes_api().get_volume(VOLUME_1_UUID)
    assert volume["spec"]["target"]["node"] == cordoned_node_id


@then("new resources of any type can be scheduled on the uncordoned node")
def new_resources_of_any_type_can_be_scheduled_on_the_uncordoned_node(
    uncordoned_resources,
):
    """new resources of any type can be scheduled on the uncordoned node."""
    uncordoned_node_id = get_node_id(uncordoned_resources[NODE_UNCORDONED_RESOURCE_KEY])
    replica_size = 20971520
    try:
        # Create a pool
        ApiClient.pools_api().put_node_pool(
            uncordoned_node_id,
            POOL_2_UUID,
            CreatePoolBody(["aio:///host/tmp/{}".format(POOL_DISK2)]),
        )

        # Create a replica and share it
        ApiClient.replicas_api().put_node_pool_replica(
            uncordoned_node_id,
            POOL_2_UUID,
            REPLICA_2_UUID,
            CreateReplicaBody(replica_size, False),
        )
        replica_uri = ApiClient.replicas_api().put_node_pool_replica_share(
            uncordoned_node_id, POOL_2_UUID, REPLICA_2_UUID
        )

        # Use the above replica to create a nexus of the same size
        ApiClient.nexuses_api().put_node_nexus(
            uncordoned_node_id,
            NEXUS_2_UUID,
            CreateNexusBody([replica_uri], replica_size),
        )

        # Create a volume
        ApiClient.volumes_api().put_volume(
            VOLUME_2_UUID, CreateVolumeBody(VolumePolicy(False), 1, VOLUME_SIZE)
        )

        # Publish the volume to ensure a nexus is created on the node
        ApiClient.volumes_api().put_volume_target(
            VOLUME_2_UUID, NODE_1_NAME, Protocol("nvmf")
        )
    except:
        assert False, "Should be able to create resources on the uncordoned node"


@then("new resources of any type cannot be scheduled on the cordoned node")
def new_resources_of_any_type_cannot_be_scheduled_on_the_cordoned_node(
    cordoned_resources,
):
    """new resources of any type cannot be scheduled on the cordoned node."""
    cordoned_node_id = get_node_id(cordoned_resources[NODE_CORDONED_RESOURCE_KEY])
    replica_size = 20971520

    # Expect pool creation to fail
    try:
        ApiClient.pools_api().put_node_pool(
            cordoned_node_id,
            POOL_2_UUID,
            CreatePoolBody(["aio:///host/tmp/{}".format(POOL_DISK2)]),
        )
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["precondition_failed"]

    # Expect creation of a replica on the existing pool to fail
    try:
        ApiClient.replicas_api().put_node_pool_replica(
            cordoned_node_id,
            POOL_1_UUID,
            REPLICA_2_UUID,
            CreateReplicaBody(replica_size, False),
        )
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["precondition_failed"]

    # Expect nexus creation using an existing replica to fail
    try:
        replica = ApiClient.replicas_api().get_replica(REPLICA_1_UUID)
        ApiClient.nexuses_api().put_node_nexus(
            cordoned_node_id,
            NEXUS_1_UUID,
            CreateNexusBody([replica["uuid"]], replica_size),
        )
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["precondition_failed"]

    # Expect volume creation to fail
    try:
        ApiClient.volumes_api().put_volume(
            VOLUME_2_UUID, CreateVolumeBody(VolumePolicy(False), 1, VOLUME_SIZE)
        )
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["insufficient_storage"]


@then(
    "resources that existed on the cordoned node prior to the restart should be recreated on the same cordoned node"
)
def resources_that_existed_on_the_cordoned_node_prior_to_the_restart_should_be_recreated_on_the_same_cordoned_node(
    cordoned_resources,
):
    """resources that existed on the cordoned node prior to the restart should be recreated on the same cordoned node."""
    # Check the restarted node is still cordoned
    cordoned_node_id = get_node_id(cordoned_resources[NODE_CORDONED_RESOURCE_KEY])
    cordoned_nodes = get_cordoned_nodes()
    node = list(
        filter(
            lambda node: node["id"] == cordoned_node_id,
            cordoned_nodes,
        )
    )
    assert len(node) == 1

    # Check the resources still exist on the node
    # We don't check for the nexus resource because the core agent doesn't recreate this upon restart.
    check_node_resources(
        cordoned_node_id, POOL_1_UUID, REPLICA_1_UUID, NEXUS_1_UUID, VOLUME_1_UUID, True
    )


@then("the cordoned node should remain cordoned")
def the_cordoned_node_should_remain_cordoned(cordoned_resources):
    """the cordoned node should remain cordoned."""
    cordoned_nodes = get_cordoned_nodes()
    # Get the node which matches the cordoned node
    node = list(
        filter(
            lambda node: node["id"]
            == get_node_id(cordoned_resources[NODE_CORDONED_RESOURCE_KEY]),
            cordoned_nodes,
        )
    )
    assert len(node) == 1

    node_cordon_labels = get_cordon_labels(node[0])
    assert len(node_cordon_labels) > 0
    assert CORDON_LABEL_1 in node_cordon_labels


@then("the resources should be deleted")
def the_resources_should_be_deleted():
    """the resources should be deleted."""
    try:
        ApiClient.volumes_api().get_volume(VOLUME_1_UUID)
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["not_found"]

    try:
        ApiClient.nexuses_api().get_nexus(NEXUS_1_UUID)
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["not_found"]

    try:
        ApiClient.replicas_api().get_replica(REPLICA_1_UUID)
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["not_found"]

    try:
        ApiClient.pools_api().get_pool(POOL_1_UUID)
    except Exception as e:
        exception_info = e.__dict__
        assert exception_info["status"] == requests.codes["not_found"]


@then("the uncordoned node should remain uncordoned")
def the_uncordoned_node_should_remain_uncordoned(uncordoned_resources):
    """the uncordoned node should remain uncordoned."""
    uncordoned_node_id = get_node_id(uncordoned_resources[NODE_UNCORDONED_RESOURCE_KEY])
    node = ApiClient.nodes_api().get_node(uncordoned_node_id)
    assert len(get_cordon_labels(node)) == 0


@then("the volume will remain in a degraded state")
def the_volume_will_remain_in_a_degraded_state():
    """the volume will remain in a degraded state."""
    try:
        wait_degraded_volume(VOLUME_2_UUID)
    except:
        # Expect the volume to remain degraded
        return


# Return a list of uncordoned nodes
def get_uncordoned_nodes():
    nodes = ApiClient.nodes_api().get_nodes()
    return list(filter(lambda node: len(get_cordon_labels(node)) == 0, nodes))


# Return a list of cordoned nodes
def get_cordoned_nodes():
    nodes = ApiClient.nodes_api().get_nodes()
    return list(filter(lambda node: len(get_cordon_labels(node)) > 0, nodes))


# Return the node ID of the given node
def get_node_id(node):
    return node["spec"]["id"]


# Return the cordon labels associated with the node
def get_cordon_labels(node):
    return node["spec"]["cordon_labels"]


# Apply the cordon label to the node and return the cordoned node
def cordon_node(node_id, cordon_label):
    # Cordon the node with resources on it
    ApiClient.nodes_api().put_node_cordon(node_id, cordon_label)
    nodes = get_cordoned_nodes()
    assert len(nodes) == 1
    return nodes[0]


# Returns true if the node with `node_id` is cordoned
def node_cordoned(node_id):
    nodes = get_cordoned_nodes()
    node = list(
        filter(
            lambda node: node["id"] == node_id,
            nodes,
        )
    )
    assert len(node) == 1
    return get_node_id(node[0]) == node_id


# Create various resources on the node with the given `node_id`
def create_node_resources(node_id, pool_id, replica_id, nexus_id, volume_id):
    replica_size = 20971520  # 20MiB
    try:
        # Create a pool
        ApiClient.pools_api().put_node_pool(
            node_id, pool_id, CreatePoolBody(["aio:///host/tmp/{}".format(POOL_DISK1)])
        )

        # Create a replica and share it
        ApiClient.replicas_api().put_node_pool_replica(
            node_id, pool_id, replica_id, CreateReplicaBody(replica_size, False)
        )
        replica_uri = ApiClient.replicas_api().put_node_pool_replica_share(
            node_id, pool_id, replica_id
        )

        # Use the above replica to create a nexus of the same size
        ApiClient.nexuses_api().put_node_nexus(
            node_id, nexus_id, CreateNexusBody([replica_uri], replica_size)
        )

        # Create a volume
        ApiClient.volumes_api().put_volume(
            volume_id, CreateVolumeBody(VolumePolicy(False), 1, VOLUME_SIZE)
        )

        # Publish the volume to ensure a nexus is created on the node
        ApiClient.volumes_api().put_volume_target(volume_id, node_id, Protocol("nvmf"))
    except Exception as e:
        assert False, "Unable to create all resources on node {}. Error {}".format(
            node_id, e
        )


# Return true if all resources exist on the node
@retry(wait_fixed=1000, stop_max_attempt_number=3)
def check_node_resources(
    node_id, pool_id, replica_id, nexus_id, volume_id, omit_nexus_check
):
    # Check the pool exists
    ApiClient.pools_api().get_node_pool(node_id, pool_id)
    # Check the replica exists
    ApiClient.replicas_api().get_node_pool_replica(node_id, pool_id, replica_id)
    if not omit_nexus_check:
        # Check the nexus exists
        ApiClient.nexuses_api().get_node_nexus(node_id, nexus_id)
    # Check the volume exists
    ApiClient.volumes_api().get_volume(volume_id)


@retry(wait_fixed=1000, stop_max_attempt_number=15)
def wait_degraded_volume(volume_id):
    volume = ApiClient.volumes_api().get_volume(volume_id)
    assert volume["state"]["status"] == VolumeStatus("Degraded")
