# Make the camera work

By the end of this page the scan screen can use the phone camera.

`getUserMedia` **requires a secure context**. Serving the application on `http://192.168.1.x:6666` makes the browser refuse the camera, without a clear message, and the product stops working. This is not a comfort detail.

!!! warning "Without a secure context"
    The scan screen stays usable: the **handheld scanner** and **manual entry** still work, and the application shows an explicit message instead of a black viewfinder.

Three situations, three answers.

## Publish through a Cloudflare tunnel

This is the reference setup. The tunnel publishes `https://qr.exemple.fr` to `http://<this machine>:6666`, and it provides the certificate. Caddy serves plain HTTP behind it.

```
Browser --https--> Cloudflare --tunnel--> 192.168.x.x:6666 --> Caddy --> app
```

Nothing to configure in the application except `DOMAINE_PUBLIC` and `DERRIERE_PROXY=true` (the default), so that the real client address is read from `CF-Connecting-IP`. That address feeds [rate limiting](security.md#rate-limiting).

On the Cloudflare side, the public service must point to `http://<ip>:6666`.

## Point a domain name straight at the machine

Edit `docker/Caddyfile`:

```caddyfile
{
    servers { trusted_proxies static private_ranges }
}

{$DOMAINE_PUBLIC} {
    encode zstd gzip
    reverse_proxy app:8080
}
```

Publish ports 80 and 443 instead of 6666. Caddy obtains and renews the certificate by itself. Keep the `caddy_data` volume: it holds the certificates.

## Test on a real phone on the local network

This is the case that traps people. `https://localhost` is a secure context, but `https://192.168.1.20` is not, without a recognised certificate.

1. **Recommended:** a tunnel, with `cloudflared tunnel --url http://localhost:6666` or `tailscale funnel 6666`. Public URL, valid certificate, no port to open, and **it works on iOS** without installing a profile.
2. **Android fallback:** `mkcert` to issue a local certificate, then install the root authority on the phone.
