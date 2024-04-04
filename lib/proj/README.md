# Proj bindings

We currently do our own because the [georust/proj](https://github.com/georust/proj) bindings requires [bindgen at compile time](https://github.com/georust/proj/issues/44) and only targets proj 9.4. E.g. ubuntu 22.04 is still on PROJ 8.2


# Updating prebuilt-bindings

Install bindgen:
```
cargo install bindgen-cli
```

Run it:
```
bindgen wrapper.h -o prebuilt-bindings/proj.rs
```

Rename the generated `proj.rs` into `proj_<PROJ_VERSION_MAJOR>_<PROJ_VERSION_MINOR>.rs` (those versions should be at the top of `proj.rs`)

