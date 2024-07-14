.PHONY: all serve display json display_small json_small display_gm_v10

all:
	cargo build --all-targets

serve:
	cargo watch -x 'run -p example-tileserver'

display:
	cargo run --bin to_npy -- example_data/local/marina_cog_nocompress_3857.tif 0 && python utils/npyshow.py img.npy

display_small:
	cargo run --bin to_npy -- example_data/example_1_cog_nocompress.tif 0 && python utils/npyshow.py img.npy

display_gm_v10:
	cargo run --bin to_npy -- example_data/local/gm_v10_3857_cog_nocompress.tif 0 && python utils/npyshow.py img.npy

json:
	cargo run -F json --bin to_json -- example_data/local/marina_cog_nocompress_3857.tif /tmp/out.json && jq . /tmp/out.json > out.json

json_small:
	cargo run -F json --bin to_json -- example_data/example_1_cog_nocompress.tif /tmp/out.json && jq . /tmp/out.json > out.json

clippy:
	cargo clippy --all-features

test:
	cargo test --all-targets