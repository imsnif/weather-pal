![img-2023-11-19-080410](https://github.com/imsnif/weather-pal/assets/795598/52f2d2e5-b9b8-4cf2-ab5e-c2554db71741)


## About

Weather-Pal is a [Zellij](https://github.com/zellij-org/zellij) plugin to show the weather for the current location

## How does it work?
Weahter-Pal uses the wonderful [open-meteo](https://open-meteo.com) API to both geocode the user's location (turn the location string into longitude and latitude) and get the weather data.

Weather-Pal *does not* geolocate the user according to their GPS/IP/Wi-Fi/Cell information.

## Try it out 
From inside Zellij:

```
zellij plugin -- https://github.com/imsnif/weather-pal/releases/latest/download/weather-pal.wasm
```

## Permanent Installation
1. Download the `weather-pal.wasm` file from the latest release
2. Place it in `~/.config/zellij/plugins`
3. From inside Zellij, run `zellij plugin [--floating] [--in-place] -- file:~/zellij/plugins/weather-pal.wasm`

## Configuration
The location can also be configured manually through the `location=<location>` plugin configuration.

eg.
```
zellij plugin --configuration location=vienna -- file:~/zellij/plugins/weather-pal.wasm
```
