# Bottlerocket Control Container

This is the control container for the [Bottlerocket](https://github.com/bottlerocket-os/bottlerocket) operating system.
This container runs the [AWS SSM agent](https://github.com/aws/amazon-ssm-agent) that lets you run commands, or start shell sessions, on Bottlerocket instances in EC2.

For more information about how the control container is used and configured with Bottlerocket, please see the [Bottlerocket documentation](https://github.com/bottlerocket-os/bottlerocket/blob/develop/README.md#control-container).

## Building the Container Image

You'll need Docker 17.06.2 or later, for multi-stage build support.
Then run `make`!
