# authorized-keys-github
Use GitHub to automatically deploy authorized key fingerprints to your servers!

## Usage

Download/build this tool, put it somewhere (like `/var/lib/authorized-keys-github`), then point to it in your `/etc/ssh/sshd_config` with something like:
```
AuthorizedKeysCommand /var/lib/authorized-keys-github --fp=%f %U
```

The tool will then check the given fingerprint and username against [GitHub's `/users/{username}/keys` endpoint](https://docs.github.com/en/rest/users/keys#list-public-keys-for-a-user).
For this tool to work, users must use the same username as their GitHub username, and they must add their SSH fingerprints to their [public GitHub SSH keys list](https://github.com/settings/keys).

## Usage Warning

Although we have taken some pains to test this in exceptional circumstances (such as disk space exhaustion, read-only filesystems, etc...) it is possible there remain serious bugs that can lock you out of your server.
We highly recommend embedding a known SSH key within a user's `~/.ssh/authorized_keys` file to be used in the initial authorization scan, as documented in [the `AuhtorizedKeysCommand` section of the `sshd_config` manpage](https://man.openbsd.org/sshd_config#AuthorizedKeysCommand).
If serious issues are discovered, please open issues and/or pull requests here and tag any of the maintainers.
Finally, we are not responsible for any damage this tool does to your system, you use it at yor own risk.
