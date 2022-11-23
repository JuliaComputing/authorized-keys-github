FROM debian

# Install `sudo` so `staticfloat` can mount things
RUN apt update -y && apt install -y sudo openssh-client ca-certificates

# Create default `staticfloat` user, give him sudo powers
ARG UID=1000
ARG GID=1000
RUN useradd -u ${UID} staticfloat
RUN echo "staticfloat ALL = NOPASSWD: ALL" >> /etc/sudoers

# Create `keno` user that will not work by default, because he's a system user
RUN useradd -u 900 keno

# Copy in our build artifacts
COPY runtests.sh /var/runtests.sh
COPY target/release/authorized-keys-github /usr/local/bin/authorized-keys-github
CMD ["/bin/bash", "/var/runtests.sh"]
USER staticfloat
