FROM public.ecr.aws/amazonlinux/amazonlinux:2 as builder

# Install build dependencies for the package(s) below
RUN \
  yum -y install \
    autoconf automake bison gettext-devel libtool make pkgconfig tar xz
COPY ./sdk-fetch /usr/local/bin

ARG utillinux_version=2.37.4

WORKDIR /opt/build
COPY ./hashes/util-linux ./hashes

RUN \
  sdk-fetch hashes && \
  tar -xf util-linux-${utillinux_version}.tar.xz && \
  rm util-linux-${utillinux_version}.tar.xz hashes

# Build script for SSM session logging
WORKDIR /opt/build/util-linux-${utillinux_version}
RUN \
  ./autogen.sh && ./configure \
        --disable-all-programs \
        --enable-scriptutils \
    || { cat config.log; exit 1; }
RUN make -j`nproc` script
RUN cp script /opt/script
RUN \
  mkdir -p /usr/share/licenses/util-linux && cp -p \
      Documentation/licenses/COPYING.BSD-4-Clause-UC \
      Documentation/licenses/COPYING.GPL-2.0-or-later \
      Documentation/licenses/COPYING.LGPL-2.1-or-later \
    /usr/share/licenses/util-linux

FROM public.ecr.aws/amazonlinux/amazonlinux:2

# IMAGE_VERSION is the assigned version of inputs for this image.
ARG IMAGE_VERSION
ENV IMAGE_VERSION=$IMAGE_VERSION
# IMAGE_VERSION is the assigned version of inputs for this image.
ARG SSM_AGENT_VERSION
ENV SSM_AGENT_VERSION=$SSM_AGENT_VERSION

# Validation
RUN : \
    "${IMAGE_VERSION:?IMAGE_VERSION is required to build}" \
    "${SSM_AGENT_VERSION:?SSM Agent version required to build}"

LABEL "org.opencontainers.image.version"="$IMAGE_VERSION"

COPY --from=builder /opt/script /usr/bin/
COPY --from=builder /usr/share/licenses/util-linux /usr/share/licenses/util-linux
# Validate script binary
RUN /usr/bin/script &>/dev/null

# Install the arch specific build of SSM agent *and confirm that it installed* -
# yum will allow architecture-mismatched packages to not install and consider
# the run successful.
# SSM Agent is downloaded from eu-north-1 as this region gets new releases of SSM Agent first.
COPY ./hashes/ssm ./hashes
RUN \
  ARCH=$(uname -m | sed 's/aarch64/arm64/' | sed 's/x86_64/amd64/') && \
  curl -L "https://s3.eu-north-1.amazonaws.com/amazon-ssm-eu-north-1/${SSM_AGENT_VERSION}/linux_${ARCH}/amazon-ssm-agent.rpm" \
       -o "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" && \
  grep "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" hashes | sha512sum --check - && \
  yum update -y && yum install -y jq screen shadow-utils && \
  yum install -y "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" && \
  rm "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" && \
  rm -rf /var/cache/yum ./hashes && \
  rmdir /var/lib/amazon/ssm && \
  ln -snf /.bottlerocket/host-containers/current/ssm /var/lib/amazon/ssm

# Add motd explaining the control container.
RUN rm -f /etc/motd /etc/issue
COPY --chown=root:root motd /etc/
# Add custom PS1 to show you are in the control container.
ARG CUSTOM_PS1='[\u@control]\$ '
RUN echo "PS1='$CUSTOM_PS1'" > "/etc/profile.d/bottlerocket-ps1.sh"
# Add bashrc that shows the motd.
COPY ./bashrc /etc/skel/.bashrc
# SSM starts sessions with 'sh', not 'bash', which for us is a link to bash.
# Furthermore, it starts sh as an interactive shell, but not a login shell.
# In this mode, the only startup file respected is the one pointed to by the
# ENV environment variable.  Point it to our bashrc, which just prints motd.
ENV ENV /etc/skel/.bashrc

# Add our helpers to quickly interact with the admin container.
COPY --chmod=755 \
  ./disable-admin-container \
  ./enable-admin-container \
  ./enter-admin-container \
  /usr/bin/

# Create our user in the group that allows API access.
RUN groupadd -g 274 api
RUN useradd -m -G users,api ssm-user

COPY --chmod=755 start_control_ssm.sh /usr/sbin/
CMD ["/usr/sbin/start_control_ssm.sh"]
