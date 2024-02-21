.PHONY: display json display_small json_small

all:
	cargo build --all-targets

display:
	cargo run --bin to_npy -- example_data/local/marina_cog_nocompress_3857.tif 0 && python utils/npyshow.py img.npy

display_small:
	cargo run --bin to_npy -- example_data/example_1_cog_nocompress.tif 0 && python utils/npyshow.py img.npy

json:
	cargo run -F json --bin to_json -- example_data/local/marina_cog_nocompress_3857.tif /tmp/out.json && jq . /tmp/out.json > out.json

json_small:
	cargo run -F json --bin to_json -- example_data/example_1_cog_nocompress.tif /tmp/out.json && jq . /tmp/out.json > out.json

clippy:
	cargo clippy --all-features
