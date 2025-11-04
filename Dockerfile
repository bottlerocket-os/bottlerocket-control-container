FROM public.ecr.aws/amazonlinux/amazonlinux:2023 AS builder

RUN dnf install -y \
    'dnf-command(download)' \
    cpio

WORKDIR /root/build/util-linux
RUN dnf download util-linux && \
    rpm2cpio util-linux-*.rpm | cpio -idmv

FROM public.ecr.aws/amazonlinux/amazonlinux:2023

# IMAGE_VERSION is the assigned version from input for this image.
ARG IMAGE_VERSION
ENV IMAGE_VERSION=$IMAGE_VERSION

# SSM_AGENT_VERSION is the assigned agent version from input for this image.
ARG SSM_AGENT_VERSION
ENV SSM_AGENT_VERSION=$SSM_AGENT_VERSION

# Validation
RUN : \
    "${IMAGE_VERSION:?IMAGE_VERSION is required to build}" \
    "${SSM_AGENT_VERSION:?SSM Agent version required to build}"

LABEL "org.opencontainers.image.version"="$IMAGE_VERSION"

# Install the arch specific build of SSM agent *and confirm that it installed* -
# dnf will allow architecture-mismatched packages to not install and consider
# the run successful.
# SSM Agent is downloaded from eu-north-1 as this region gets new releases of SSM Agent first.
COPY ./hashes/ssm ./hashes
COPY ./gpg-keys/amazon-ssm-agent.gpg ./amazon-ssm-agent.gpg
RUN dnf update -y && \
    dnf install -y \
        crypto-policies-scripts \
        jq \
        libutempter \
        screen \
        shadow-utils \
        && \
    dnf remove -y amazon-ssm-agent && \
    ARCH=$(uname -m | sed 's/aarch64/arm64/' | sed 's/x86_64/amd64/') && \
    curl -L "https://s3.eu-north-1.amazonaws.com/amazon-ssm-eu-north-1/${SSM_AGENT_VERSION}/linux_${ARCH}/amazon-ssm-agent.rpm" \
        -o "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" && \
    grep "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" hashes \
        | sha512sum --check - && \
    rpm --import amazon-ssm-agent.gpg && \
    rpm --checksig "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" && \
    dnf install -y "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" && \
    rm "amazon-ssm-agent-${SSM_AGENT_VERSION}.${ARCH}.rpm" && \
    rm -rf /var/cache/dnf ./hashes && \
    rmdir /var/lib/amazon/ssm && \
    ln -snf /.bottlerocket/host-containers/current/ssm /var/lib/amazon/ssm

# Copy util-linux binaries and dependencies
COPY --from=builder /root/build/util-linux/usr/bin/lscpu /root/build/util-linux/usr/bin/script \
                    /opt/util-linux/bin/
COPY --from=builder /root/build/util-linux/usr/share/licenses/util-linux/COPYING.BSD-4-Clause-UC \
                    /root/build/util-linux/usr/share/licenses/util-linux/COPYING.GPL-2.0-or-later \
                    /root/build/util-linux/usr/share/licenses/util-linux/COPYING.LGPL-2.1-or-later \
                    /usr/share/licenses/util-linux/
RUN ln -s /opt/util-linux/bin/* /usr/bin

# Validate lscpu binary
RUN /usr/bin/lscpu
# Validate script binary
RUN /usr/bin/script --version

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
ENV ENV=/etc/skel/.bashrc

# Add our helpers to quickly interact with the admin container.
COPY --chmod=755 \
    ./disable-admin-container \
    ./enable-admin-container \
    ./enter-admin-container \
    /usr/bin/

# Create our user in the group that allows API access.
RUN groupadd -g 274 api && \
    useradd -m -G users,api ssm-user

COPY --chmod=755 start_control_ssm.sh /usr/sbin/
CMD ["/usr/sbin/start_control_ssm.sh"]
