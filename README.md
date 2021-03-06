<img height="200" src="assets/logo.png" alt="logo">

> Turns a folder of MP3 files into a hosted RSS feed.

## Usage

```
podserve [-d podcasts/] https://pod.example.com/prefix/
```

By default, `podserve` will read the MP3s in a `podcasts/` subdirectory
and serve them, generating an absolute URL based on the prefix URL
provided as first positional argument.

`podserve` will extract ID3 tags and modification time to generate
the corresponding attributes on the RSS feed.

## Advanced Configuration

You can set some additional configuration parameters through a config file.
To write a template with default values, run `podserve` with `--write-config`
and specify the edited configuration when starting it later with `--config`.

```
podserve https://example.com/ --write-config config.toml
$VISUAL ./config.toml
podserve https://example.com/ --config config.toml
```

*Yes, the URL is required here for no reason other than to make my life easier.
PRs welcome.*

## Serving Static Files

`podserve` uses Rocket and the Rocket-Contrib static file serving mechanism.
This, importantly, does not support range requests and is hence very ill-suited
for streaming podcasts. You may want to put a reverse proxy in front of this
(which does of course takes the fun a bit out of this).

For nginx you may want to set up something like this:

```nginx
location /podcasts/ {
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 65;

    alias /srv/www/podserve/podcasts/;
}

location / {
    proxy_pass http://127.0.0.1:{{ podserve_port }};
}
```

## Caveats

This is by no means meant to be a used in production or production-like
environments. There are deliberately few options to configure your feed.
I'm happy to take Pull Requests to add more, but this is not meant to be
feature-complete.

The scenario I'm envisioning for this is that you have a bunch of drafts
you quickly want to share with your team or friends and instead of sharing
a Dropbox folder, you instead share an actual feed that people can subscribe
to with their podcast players. This comes with the additional benefit that
they can download it to listen to it later and apply the usual audio filters
on on it.

## LICENSE

[MIT](/LICENSE)
