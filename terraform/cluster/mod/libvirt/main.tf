# variables are declared in the top level variable file

variable "image_path" {}
variable "num_nodes" {}
variable "worker_memory" {}
variable "worker_vcpu" {}
variable "master_memory" {}
variable "master_vcpu" {}
variable "hostname_formatter" {}
variable "ssh_user" {}
variable "ssh_key" {}
variable "private_key_path" {}
variable "disk_size" {}
variable "pooldisk_size" {}
variable "qcow2_image" {}
variable "network_mode" {}
variable "bridge_name" {}

provider "libvirt" {
  uri = "qemu:///system"
}

# a pool is where we store all images and cloud-init iso's
resource "libvirt_pool" "ubuntu-pool" {
  name = "ubuntu-pool"
  type = "dir"
  path = var.image_path
}

# our base image from ubuntu, that includes by default cloud-init
resource "libvirt_volume" "ubuntu-qcow2" {
  name   = "ubuntu-base"
  pool   = libvirt_pool.ubuntu-pool.name
  source = var.qcow2_image
  format = "qcow2"
}

# we want to, based of the first image, create 3 separate images each with their own cloud-init settings
resource "libvirt_volume" "ubuntu-qcow2-resized" {
  name           = format(var.hostname_formatter, count.index + 1)
  count          = var.num_nodes
  base_volume_id = libvirt_volume.ubuntu-qcow2.id
  pool           = libvirt_pool.ubuntu-pool.name
  size           = var.disk_size
}

# pool disk
resource "libvirt_volume" "pool-disk" {
  name   = format("pooldisk-%s", count.index + 1)
  count  = var.num_nodes
  pool   = libvirt_pool.ubuntu-pool.name
  format = "raw"
  size   = var.pooldisk_size
}

# user data that we pass to cloud init that reads variables from variables.tf and
# passes them to a template file to be filled in.

data "template_file" "user_data" {
  count = var.num_nodes
  template = templatefile("${path.module}/cloud_init.tmpl",
    { ssh_user = var.ssh_user, ssh_key = var.ssh_key,
  hostname = format(var.hostname_formatter, count.index + 1) })
}

# likewise for networking
data "template_file" "network_config" {
  template = file("${path.module}/network_config.cfg")
}

resource "libvirt_network" "kube_network_nat" {
  count = var.network_mode == "nat" ? 1 : 0

  # the name used by libvirt
  name = "k8snet"

  # mode can be: "nat" (default), "none", "route", "bridge"
  mode = var.network_mode

  # the domain used by the DNS server in this network
  domain = "k8s.local"

  # list of subnets the addresses allowed for domains connected
  # also derived to define the host addresses
  # also derived to define the addresses served by the DHCP server
  addresses = ["10.0.0.0/24"]

  # (Optional) Set to true to start the network on host boot up.
  # If not specified false is assumed.
  autostart = "true"

  # (Optional) DNS configuration
  dns {
    # (Optional, default false)
    # Set to true, if no other option is specified and you still want to
    # enable dns.
    enabled = true
    # (Optional, default false)
    # true: DNS requests under this domain will only be resolved by the
    # virtual network's own DNS server
    # false: Unresolved requests will be forwarded to the host's
    # upstream DNS server if the virtual network's DNS server does not
    # have an answer.
    local_only = true
  }
}

resource "libvirt_network" "kube_network_bridge" {
  count  = var.network_mode == "bridge" ? 1 : 0
  bridge = var.bridge_name

  name      = "k8snet"
  mode      = "bridge"
  autostart = "true"
}

# our cloud-init disk resource
resource "libvirt_cloudinit_disk" "commoninit" {
  name           = format("commoninit-%d.iso", count.index + 1)
  count          = var.num_nodes
  user_data      = data.template_file.user_data[count.index].rendered
  network_config = data.template_file.network_config.rendered
  pool           = libvirt_pool.ubuntu-pool.name
}

# create the actual VMs for the cluster
resource "libvirt_domain" "ubuntu-domain" {
  count     = var.num_nodes
  name      = format(var.hostname_formatter, count.index + 1)
  memory    = count.index == 0 ? var.master_memory : var.worker_memory
  vcpu      = count.index == 0 ? var.master_vcpu : var.worker_vcpu
  autostart = true

  cloudinit = libvirt_cloudinit_disk.commoninit[count.index].id

  cpu {
    mode = "host-passthrough"
  }

  disk {
    volume_id = libvirt_volume.ubuntu-qcow2-resized[count.index].id
    scsi      = "true"
  }

  disk {
    volume_id = libvirt_volume.pool-disk[count.index].id
    scsi      = "true"
  }

  console {
    type        = "pty"
    target_type = "serial"
    target_port = "0"
  }

  network_interface {
    network_name   = var.network_mode == "default" ? "default" : "k8snet"
    hostname       = format(var.hostname_formatter, count.index + 1)
    wait_for_lease = true
  }

  console {
    type        = "pty"
    target_type = "virtio"
    target_port = "1"
  }

  # as each nodes comes online, grab the DHCP assigned IP and call cloud-init status --wait
  # this will keep running until SSH allows access and thus, we know by then, the system
  # is ready for business as it would return only when cloud-init has completed. We do not however
  # know the outcome of cloud-init but we have faith, and will know soon enough if it failed

  provisioner "remote-exec" {
    inline = ["cloud-init status --wait"]
    connection {
      type        = "ssh"
      user        = var.ssh_user
      host        = self.network_interface.0.addresses.0
      private_key = file(var.private_key_path)
    }
  }
}

# generate the inventory template for ansible
output "ks-cluster-nodes" {
  value = <<EOT
[master]
${libvirt_domain.ubuntu-domain.0.name} ansible_host=${libvirt_domain.ubuntu-domain.0.network_interface.0.addresses.0} ansible_user=${var.ssh_user} ansible_ssh_private_key_file=${var.private_key_path} ansible_ssh_common_args='-o StrictHostKeyChecking=no'

[nodes]%{for ip in libvirt_domain.ubuntu-domain.*~}
%{if ip.name != "${format(var.hostname_formatter, 1)}"}${ip.name} ansible_host=${ip.network_interface.0.addresses.0} ansible_user=${var.ssh_user} ansible_ssh_private_key_file=${var.private_key_path} ansible_ssh_common_args='-o StrictHostKeyChecking=no'%{endif}
%{endfor~}
EOT
}

output "node_list" {
  value = libvirt_domain.ubuntu-domain.*.network_interface.0.addresses.0
}

terraform {
  required_version = ">= 0.13"

  required_providers {
    libvirt = {
      source  = "dmacvicar/libvirt"
      version = ">= 0.6.11"
    }
  }
}
