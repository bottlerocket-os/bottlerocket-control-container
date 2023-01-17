# Bottlerocket Control Container

This is the control container for the [Bottlerocket](https://github.com/bottlerocket-os/bottlerocket) operating system.
This container runs the [AWS SSM Agent](https://github.com/aws/amazon-ssm-agent) that lets you run commands, or start interactive sessions, on Bottlerocket instances in EC2 and hybrid environments.

For more information about the control container, including how to use it and how to replace it or remove it from Bottlerocket, please see the [Bottlerocket documentation](https://github.com/bottlerocket-os/bottlerocket/blob/develop/README.md#control-container).

## Building the Container Image

You'll need Docker 20.10 or later for multi-stage build, BuildKit, and chmod on COPY/ADD support.
Then run `make`!

## Connecting to AWS Systems Manager (SSM)

Starting from v0.5.0, users have the option to pass in their own activation information for SSM.
This is for users that want to set up on-premises virtual machines (VMs) in their hybrid environment as managed instances.

Users can add their [own activations](https://docs.aws.amazon.com/systems-manager/latest/userguide/sysman-managed-instance-activation.html) by populating the control container's user data with a base64-encoded JSON block.

To use hybrid activations for managed instances you will want to generate a JSON-structure like this:

```json
{
  "ssm": {
    "activation-id": "foo",
    "activation-code": "bar",
    "region":"us-west-2"
  }
}
```

Once you've created your JSON, you'll need to base64-encode it and put it in the control host container's `user-data` setting in your [instance user data](https://github.com/bottlerocket-os/bottlerocket#using-user-data).

For example:

```toml
[settings.host-containers.control]
# ex: echo '{"ssm":{"activation-id":"foo","activation-code":"bar","region":"us-west-2"}}' | base64
user-data = "eyJzc20iOnsiYWN0aXZhdGlvbi1pZCI6ImZvbyIsImFjdGl2YXRpb24tY29kZSI6ImJhciIsInJlZ2lvbiI6InVzLXdlc3QtMiJ9fQo="
```
