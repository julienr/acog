.PHONY: all serve display json display_small json_small display_gm_v10 fmt lint

all:
	cargo build --all-targets

serve:
	cargo watch -x 'run -p example-tileserver'

display:
	rm -f img.ppm && cargo run --bin to_ppm -- example_data/local/marina_cog_nocompress_3857.tif 0 && eog img.ppm

display_small:
	rm -f img.ppm && cargo run --bin to_ppm -- example_data/example_1_cog_nocompress.tif 0 && eog img.ppm

display_gm_v10:
	rm -f img.ppm && cargo run --bin to_ppm -- example_data/local/gm_v10_3857_cog_nocompress.tif 0 && eog img.ppm

json:
	cargo run -F json --bin to_json -- example_data/local/marina_cog_nocompress_3857.tif /tmp/out.json && jq . /tmp/out.json > out.json

json_small:
	cargo run -F json --bin to_json -- example_data/example_1_cog_nocompress.tif /tmp/out.json && jq . /tmp/out.json > out.json

fmt:
	cargo fmt
	venv/bin/python -m black python/acog

lint:
	cargo clippy --all-features
	venv/bin/python -m flake8 python/acog

test:
	cargo test --all-targets
