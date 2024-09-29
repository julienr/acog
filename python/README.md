This contains the python bindings for acog, which are managed with maturin

https://github.com/PyO3/maturin

https://pyo3.rs/v0.21.2/

## Setup

```
cd python
python3 -m venv venv
. venv/bin/activate
pip install maturin patchelf
```

## Commands

Run `maturin develop` in `python` to build python package