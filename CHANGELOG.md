# 0.6.0

* Update SSM Agent version to 3.1.1141.0. ([#30])
* Fix missing packages to support SSM session logging. ([#27], [#28])
* Add some art to MOTD. ([#25])
* Switch to ECR Public, fix multi-arch builds, and more. ([#26])
* Improve build process by moving SSM agent install to own line. ([#29])

[#25]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/25
[#26]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/26
[#27]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/27
[#28]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/28
[#29]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/29
[#30]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/30

# 0.5.5

* Add 'disable-admin-container' to easily disable the admin container. ([#23])
* Update SSM Agent version to 3.1.821.0. ([#23])

[#23]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/23

# 0.5.4

* Add 'enter-admin-container' to Dockerfile. ([#21])

[#21]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/21

# 0.5.3

* Add 'enter-admin-container' to easily enable and enter admin container. ([#18])
* Update SSM Agent version to 3.1.501.0. ([#19])

[#18]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/18
[#19]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/19

# 0.5.2

* Update SSM Agent version to 3.1.192.0. ([#15])

[#15]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/15

# 0.5.1

* Update SSM Agent version to 3.0.1209.0. ([#14])

[#14]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/14

# 0.5.0

* Add support for on-premises and hybrid environments. ([#12])
* Symlinked `/var/lib/amazon/ssm` to `/.bottlerocket/host-containers/current/ssm` so that SSM Agent state data can persist between boots. ([#12])
* Update SSM Agent version to 3.0.882.0. ([#12])

[#12]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/12

# 0.4.2

* Update SSM Agent version to 3.0.732.0. ([#8])

[#8]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/8

# 0.4.1

* Update SSM Agent version to 2.3.1569.0. ([#5])

[#5]: https://github.com/bottlerocket-os/bottlerocket-control-container/pull/5

# 0.4.0

Initial release of **bottlerocket-control-container** - the default control container for Bottlerocket.

See the [README](README.md) for additional information.

