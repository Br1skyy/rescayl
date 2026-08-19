#!/bin/bash

# Step 1: Fetch JSON from GitHub API and get assets_url
assets_url=$(curl -s https://api.github.com/repos/upscayl/upscayl-ncnn/releases/latest | jq -r '.assets_url')

# Step 2: Loop through each asset and download the files
curl -s $assets_url | jq -r '.[] | .browser_download_url' | while read -r download_url; do
    filename=$(basename $download_url)
    echo "Downloading $filename..."
    curl -LO $download_url
done

# Step 3: Extract downloaded ZIP files to a specific folder
mkdir -p extracted_files
for file in *.zip; do
    echo "Extracting $file..."
    unzip -q $file -d extracted_files
done

# Step 4: Move files to respective folders
for folder in extracted_files/upscayl-bin-*; do
	if [[ -d $folder ]]; then
		platform=$(echo "extracted_files/$folder" | cut -d '-' -f 5)
		echo "Moving files in $folder to $platform folder..."
		if [[ "$platform" == "linux" ]]; then
			cp "$folder"/upscayl-bin resources/linux/bin/upscayl-bin
		elif [[ "$platform" == "macos" ]]; then
			cp "$folder"/upscayl-bin resources/mac/bin/upscayl-bin
		elif [[ "$platform" == "windows" ]]; then
			cp "$folder"/upscayl-bin.exe resources/win/bin/upscayl-bin.exe
			cp "$folder"/vcomp140.dll resources/win/bin/vcomp140.dll
			cp "$folder"/vcomp140d.dll resources/win/bin/vcomp140d.dll
		fi
	fi
done

echo "All files moved to their respective folders successfully."

# Step 5: Clean up extracted_files folder and downloaded ZIP files
rm -rf extracted_files
rm -f *.zip

echo "Script executed successfully."

# Default model weights are not stored in git; they are fetched from the
# upstream Upscayl repos at build time so the bundle ships with working models.
download_default_models() {
    mkdir -p resources/models

    # Bundled upscayl models (param + bin pairs) from the Upscayl repo.
    upscayl_models="digital-art-4x high-fidelity-4x remacri-4x ultramix-balanced-4x ultrasharp-4x upscayl-lite-4x upscayl-standard-4x"
    for m in $upscayl_models; do
        echo "Downloading model $m..."
        curl -sL -o resources/models/$m.param https://raw.githubusercontent.com/upscayl/upscayl/main/resources/models/$m.param
        curl -sL -o resources/models/$m.bin https://raw.githubusercontent.com/upscayl/upscayl/main/resources/models/$m.bin
    done

    # Anime video models from the custom-models repo.
    anime_models="realesr-animevideov3-x2 realesr-animevideov3-x3 realesr-animevideov3-x4"
    for m in $anime_models; do
        echo "Downloading model $m..."
        curl -sL -o resources/models/$m.param https://raw.githubusercontent.com/upscayl/custom-models/main/models/$m.param
        curl -sL -o resources/models/$m.bin https://raw.githubusercontent.com/upscayl/custom-models/main/models/$m.bin
    done

    echo "Models downloaded successfully."
}

download_default_models