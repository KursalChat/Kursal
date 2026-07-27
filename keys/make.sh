mkdir -p ./keys

if [ ! -f ./keys/publishing.key ]; then
    echo "\n\n=> Publishing key not found!"
    echo "You will have to enter the same password thrice\n"

    read -s -p "Enter Publishing Password: " password
    printf '%s' "$password" > ./keys/publishing.key.pwd
    echo

    cd kursal-tauri
    bunx tauri signer generate -w ../keys/publishing.key

    pubkey=$(cat ../keys/publishing.key.pub)

    jq --arg pubkey "$pubkey" '.plugins.updater.pubkey = $pubkey' ./src-tauri/tauri.conf.json > tmp.json
    mv tmp.json ./src-tauri/tauri.conf.json

    cd ..
fi


if [ ! -f ./keys/android.jks ]; then
    echo "\n\n=> Android signing key not found!"
    echo "You will have to enter the same password thrice\n"

    read -s -p "Enter Signing Password: " password
    echo

    keytool -genkey -v -keystore ./keys/android.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload

    echo "storeFile=$PWD/keys/android.jks\npassword=\"$password\"\nkeyAlias=upload" > ./kursal-tauri/src-tauri/gen/android/keystore.properties
fi

echo "\n\n=> All good!"