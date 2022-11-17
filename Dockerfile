FROM debian

# Install `sudo` so `staticfloat` can mount things
RUN apt update -y && apt install -y sudo openssh-client ca-certificates

# Create default `staticfloat` user, give him sudo powers
ARG UID=1000
ARG GID=1000
RUN useradd -u ${UID} staticfloat
RUN echo "staticfloat ALL = NOPASSWD: ALL" >> /etc/sudoers

# Copy in our build artifacts
COPY runtests.sh /var/runtests.sh
COPY target/release/authorized-keys-github /usr/local/bin/authorized-keys-github
CMD ["/bin/bash", "/var/runtests.sh"]
USER staticfloat
