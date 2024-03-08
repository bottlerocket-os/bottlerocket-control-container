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
SSM_AGENT_VERSION ?= 3.3.131.0

.PHONY: all build check check-ssm-agent download-ssm-agent update-ssm-agent

# Run all build tasks for this container image.
all: build check

# Create a distribution container image tarball for release.
dist: all
	@mkdir -p $(dir $(DISTFILE))
	docker save $(IMAGE_NAME) | gzip > $(DISTFILE)

# Build the container image.
build:
	DOCKER_BUILDKIT=1 docker build $(DOCKER_BUILD_FLAGS) \
		--tag $(IMAGE_NAME) \
		--build-arg IMAGE_VERSION="$(IMAGE_VERSION)" \
		--build-arg SSM_AGENT_VERSION="$(SSM_AGENT_VERSION)" \
		-f Dockerfile . >&2

# Run checks against the container image.
check: check-ssm-agent

# Check that the SSM Agent is the expected version.
check-ssm-agent:
	docker run --rm --entrypoint /usr/bin/bash \
		$(IMAGE_NAME) \
		-c 'rpm -q amazon-ssm-agent --queryformat "%{version}\n" | grep -qFw "$(SSM_AGENT_VERSION)"' >&2

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
