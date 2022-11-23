# authorized-keys-github
Use GitHub to automatically deploy authorized key fingerprints to your servers!

## Usage

Download/build this tool, put it somewhere (like `/var/lib/authorized-keys-github`), then point to it in your `/etc/ssh/sshd_config` with something like:
```
AuthorizedKeysCommand /var/lib/authorized-keys-github --fp=%f %U
```

The tool will then check the given fingerprint and username against [GitHub's `/users/{username}/keys` endpoint](https://docs.github.com/en/rest/users/keys#list-public-keys-for-a-user).
For this tool to work, users must use the same username as their GitHub username, and they must add their SSH fingerprints to their [public GitHub SSH keys list](https://github.com/settings/keys).

The tool will attempt to build a cache of SSH key fingerprints at `/var/keys` by default; to change this, use the `--keys-dir` option.

## Overriding local usernames and usage for system accounts

In the event that you want to use this tool with local usernames that you do not control (such as the `root` user on a single-user system like OpenWRT) you can use the `--overrides-file` argument to explicitly map UIDs to GitHub usernames.
A simple example is given here:
```
# Allow users `keno` and `staticfloat` to login as `root`
0: keno staticfloat
```
A more complete example overrides file with comments is [given in `example.overrides`](./example.overrides).
Note that overrides files must be owned by the user that the command is being run by (usually `root`) and cannot be writable by any other user or group.
In order to actually login as `root` (or any system user account with a UID <1000) you must explicitly whitelist the account by passing `--allow-system-uid=xxx` to the command.

## Usage Warning

Although we have taken some pains to test this in exceptional circumstances (such as disk space exhaustion, read-only filesystems, etc...) it is possible there remain serious bugs that can lock you out of your server.
We highly recommend embedding a known SSH key within a user's `~/.ssh/authorized_keys` file to be used in the initial authorization scan, as documented in [the `AuhtorizedKeysCommand` section of the `sshd_config` manpage](https://man.openbsd.org/sshd_config#AuthorizedKeysCommand).
If serious issues are discovered, please open issues and/or pull requests here and tag any of the maintainers.
Finally, we are not responsible for any damage this tool does to your system, you use it at yor own risk.
