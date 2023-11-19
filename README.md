## About

Weather-Pal is a [Zellij](https://github.com/zellij-org/zellij) plugin to show the weather for the current location

## How does it work?
Weahter-Pal uses the wonderful [open-meteo](https://open-meteo.com) API to both geocode the user's location (turn the location string into longitude and latitude) and get the weather data.

Weather-Pal *does not* geolocate the user according to their GPS/IP/Wi-Fi/Cell information.

## Try it out 
From inside Zellij:

```
zellij plugin -- https://github.com/imsnif/weather-pal/releases/latest/download/monocle.wasm
```

## Permanent Installation
1. Download the `weather-pal.wasm` file from the latest release
2. Place it in `~/.config/zellij/plugins`
3. From inside Zellij, run `zellij plugin [--floating] [--in-place] -- file:~/zellij/plugins/weather-pal.wasm`
