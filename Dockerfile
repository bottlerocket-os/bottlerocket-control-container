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
  yum -y update && yum install -y "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" shadow-utils jq && \
  rm "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" && \
  rm -rf /var/cache/yum ./hashes && \
  rmdir /var/lib/amazon/ssm && \
  ln -snf /.bottlerocket/host-containers/current/ssm /var/lib/amazon/ssm

# Add motd explaining the control container.
RUN rm -f /etc/motd /etc/issue
COPY --chown=root:root motd /etc/
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
