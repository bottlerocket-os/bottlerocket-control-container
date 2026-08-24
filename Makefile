TOP := $(dir $(abspath $(firstword $(MAKEFILE_LIST))))

# IMAGE_NAME is the full name of the container image being built.
IMAGE_NAME ?= $(notdir $(shell pwd -P))$(IMAGE_ARCH_SUFFIX):$(IMAGE_VERSION)$(addprefix -,$(SHORT_SHA))
# IMAGE_VERSION is the semver version that's tagged on the image.
IMAGE_VERSION = $(shell cat VERSION)
# SHORT_SHA is the revision that the container image was built with.
SHORT_SHA ?= $(shell git describe --abbrev=8 --always --dirty='-dev' --exclude '*' || echo "unknown")
# IMAGE_ARCH_SUFFIX is the runtime architecture designator for the container
# image, it is appended to the IMAGE_NAME unless the name is specified.
IMAGE_ARCH_SUFFIX ?= $(addprefix -,$(ARCH))
# DESTDIR is where the release artifacts will be written.
DESTDIR ?= .
# DISTFILE is the path to the dist target's output file - the container image
# tarball.
DISTFILE ?= $(subst /,,$(DESTDIR))/$(subst /,_,$(IMAGE_NAME)).tar.gz

UNAME_ARCH = $(shell uname -m)
ARCH ?= $(lastword $(subst :, ,$(filter $(UNAME_ARCH):%,x86_64:amd64 aarch64:arm64)))

# SSM_AGENT_VERSION is the SSM Agent's distributed RPM Version to install.
SSM_AGENT_VERSION ?= 3.3.5226.0

# BOTTLEROCKET_SDK_VERSION is the SDK image used to build corgid.
BOTTLEROCKET_SDK_VERSION ?= v0.73.0

.PHONY: all build check check-ssm-agent check-licenses fetch fmt clippy test download-ssm-agent update-ssm-agent

# Run all build tasks for this container image.
all: build check

# Fetches crates from upstream
fetch:
	docker run --rm \
		--user "$(shell id -u):$(shell id -g)" \
		--security-opt label=disable \
		--env CARGO_HOME="/src/.cargo" \
		--volume "$(TOP)/sources:/src" \
		--workdir "/src/" \
		"public.ecr.aws/bottlerocket/bottlerocket-sdk:$(BOTTLEROCKET_SDK_VERSION)" \
		bash -c "cargo fetch --locked --manifest-path /src/corgid/Cargo.toml"

# Checks allowed/denied upstream licenses
check-licenses: fetch
	docker run --rm \
		--network none \
		--user "$(shell id -u):$(shell id -g)" \
		--security-opt label=disable \
		--env CARGO_HOME="/src/.cargo" \
		--volume "$(TOP)/sources:/src" \
		--workdir "/src/" \
		"public.ecr.aws/bottlerocket/bottlerocket-sdk:$(BOTTLEROCKET_SDK_VERSION)" \
		bash -c "cd /src/corgid && cargo deny --all-features check --disable-fetch licenses bans sources"

# Check code formatting
fmt: fetch
	docker run --rm \
		--user "$(shell id -u):$(shell id -g)" \
		--security-opt label=disable \
		--env CARGO_HOME="/src/.cargo" \
		--volume "$(TOP)/sources:/src" \
		--workdir "/src/" \
		"public.ecr.aws/bottlerocket/bottlerocket-sdk:$(BOTTLEROCKET_SDK_VERSION)" \
		bash -c "cargo fmt --manifest-path /src/corgid/Cargo.toml -- --check"

# Run clippy lints
clippy: fetch
	docker run --rm \
		--network none \
		--user "$(shell id -u):$(shell id -g)" \
		--security-opt label=disable \
		--env CARGO_HOME="/src/.cargo" \
		--volume "$(TOP)/sources:/src" \
		--workdir "/src/" \
		"public.ecr.aws/bottlerocket/bottlerocket-sdk:$(BOTTLEROCKET_SDK_VERSION)" \
		bash -c "cargo clippy --locked --manifest-path /src/corgid/Cargo.toml -- -D warnings"

# Run unit tests
test: fetch
	docker run --rm \
		--network none \
		--user "$(shell id -u):$(shell id -g)" \
		--security-opt label=disable \
		--env CARGO_HOME="/src/.cargo" \
		--volume "$(TOP)/sources:/src" \
		--workdir "/src/" \
		"public.ecr.aws/bottlerocket/bottlerocket-sdk:$(BOTTLEROCKET_SDK_VERSION)" \
		bash -c "cargo test --locked --manifest-path /src/corgid/Cargo.toml"

# Create a distribution container image tarball for release.
dist: all
	@mkdir -p $(dir $(DISTFILE))
	docker save $(IMAGE_NAME) | gzip > $(DISTFILE)

# Build the container image.
build: check-licenses fmt clippy
	DOCKER_BUILDKIT=1 docker build $(DOCKER_BUILD_FLAGS) \
		--tag $(IMAGE_NAME) \
		--build-arg IMAGE_VERSION="$(IMAGE_VERSION)" \
		--build-arg SSM_AGENT_VERSION="$(SSM_AGENT_VERSION)" \
		--build-arg UNAME_ARCH="$(UNAME_ARCH)" \
		--build-arg SDK_IMAGE="public.ecr.aws/bottlerocket/bottlerocket-sdk:$(BOTTLEROCKET_SDK_VERSION)" \
		-f Dockerfile . >&2

# Run checks against the container image.
check: check-ssm-agent

# Check that the SSM Agent is the expected version.
check-ssm-agent:
	@echo "Running SSM version check"
	@docker run --rm --entrypoint /usr/bin/bash \
		$(IMAGE_NAME) \
		-c 'amazon-ssm-agent -version | grep -Fw "$(SSM_AGENT_VERSION)"' >&2

# Download SSM Agent version SSM_AGENT_VERSION for all architectures.
download-ssm-agent: amazon-ssm-agent-${SSM_AGENT_VERSION}.amd64.rpm amazon-ssm-agent-${SSM_AGENT_VERSION}.arm64.rpm

amazon-ssm-agent-${SSM_AGENT_VERSION}.amd64.rpm:
	curl -L "https://s3.eu-north-1.amazonaws.com/amazon-ssm-eu-north-1/${SSM_AGENT_VERSION}/linux_amd64/amazon-ssm-agent.rpm" \
		-o "amazon-ssm-agent-${SSM_AGENT_VERSION}.amd64.rpm"

amazon-ssm-agent-${SSM_AGENT_VERSION}.arm64.rpm:
	curl -L "https://s3.eu-north-1.amazonaws.com/amazon-ssm-eu-north-1/${SSM_AGENT_VERSION}/linux_arm64/amazon-ssm-agent.rpm" \
		-o "amazon-ssm-agent-${SSM_AGENT_VERSION}.arm64.rpm"

# Update the expected hashes of SSM Agent to those for SSM_AGENT_VERSION.
update-ssm-agent: download-ssm-agent
	sha512sum amazon-ssm-agent-${SSM_AGENT_VERSION}.*.rpm >hashes/ssm

clean:
	rm -f $(DISTFILE)
