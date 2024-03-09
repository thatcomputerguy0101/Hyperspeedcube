<img src="https://raw.githubusercontent.com/HactarCE/Hyperspeedcube/main/resources/icon/hyperspeedcube.svg?sanitize=true" alt="Hyperspeedcube logo" width="150" align="right">

# [Hyperspeedcube] [![Release badge]][Release link]

[Dependencies badge]: https://deps.rs/repo/github/HactarCE/Hyperspeedcube/status.svg "Dependencies status"
[Release badge]: https://img.shields.io/github/v/release/HactarCE/Hyperspeedcube
[Release link]: https://github.com/HactarCE/Hyperspeedcube/releases/latest

Hyperspeedcube is a modern, beginner-friendly 3D and 4D Rubik's cube simulator with customizable mouse and keyboard controls and advanced features for speedsolving. It's been used to break numerous speedsolving records.

For more info, see [ajfarkas.dev/hyperspeedcube](https://ajfarkas.dev/hyperspeedcube/)

[Hyperspeedcube]: https://ajfarkas.dev/hyperspeedcube/

## Project structure

This project consists of four crates, each depending on the previous ones:

- `hypermath`, which implements vectors, matrices, conformal geometric algebra primitives, and common multi-purpose data structures
- `hypershape`, which implements shape slicing and other geometric algorithms
- `hyperpuzzle`, which implements puzzle construction and simulation via a Lua API
- `hyperspeedcube`, which implements the UI frontend

### Possible future plans

- Split `hyperspeedcube` into `hyperspeedcube_core`, `hyperspeedcube_wgpu`, and `hyperspeedcube_egui`.
- Alternatively: by default, `hyperspeedcube` has the `gui` feature enabled. By disabling it, you can use `hyperspeedcube` as a dependency in other projects and build your own frontend.

### License & contributing

<!-- This is generated by the Creative Commons license chooser and is intended to be machine-readable -->
<p xmlns:cc="http://creativecommons.org/ns#" xmlns:dct="http://purl.org/dc/terms/"><a property="dct:title" rel="cc:attributionURL" href="https://ajfarkas.dev/hyperspeedcube/">Hyperspeedcube</a> by <a rel="cc:attributionURL dct:creator" property="cc:attributionName" href="https://ajfarkas.dev/">Andrew Farkas</a> is licensed under <a href="http://creativecommons.org/licenses/by-nc-sa/4.0/?ref=chooser-v1" target="_blank" rel="license noopener noreferrer" style="display:inline-block;">CC BY-NC-SA 4.0<img style="height:22px!important;margin-left:3px;vertical-align:text-bottom;" src="https://mirrors.creativecommons.org/presskit/icons/cc.svg?ref=chooser-v1"><img style="height:22px!important;margin-left:3px;vertical-align:text-bottom;" src="https://mirrors.creativecommons.org/presskit/icons/by.svg?ref=chooser-v1"><img style="height:22px!important;margin-left:3px;vertical-align:text-bottom;" src="https://mirrors.creativecommons.org/presskit/icons/nc.svg?ref=chooser-v1"><img style="height:22px!important;margin-left:3px;vertical-align:text-bottom;" src="https://mirrors.creativecommons.org/presskit/icons/sa.svg?ref=chooser-v1"></a></p>

**By contributing to this work, you agree that your contributions will be under the sole ownership of Andrew Farkas.**

If you want to use all or part of the source code for Hyperspeedcube for a purpose that is forbidden by this license, contact me and we may be able to work something out.
