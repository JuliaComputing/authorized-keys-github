FROM ubuntu

# Install `sudo` so `staticfloat` can mount things
RUN apt update -y && apt install -y sudo curl openssh-client

# Create default `staticfloat` user, give him sudo powers
RUN useradd staticfloat
RUN echo "staticfloat ALL = NOPASSWD: ALL" >> /etc/sudoers

# Copy in our build artifacts
COPY runtests.sh /var/runtests.sh
COPY target/release/authorized-keys-github /usr/local/bin/authorized-keys-github
CMD ["/bin/bash", "/var/runtests.sh"]
USER staticfloat
