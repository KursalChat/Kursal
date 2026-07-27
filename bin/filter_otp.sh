#!/bin/bash
set -e

cd kursal-core/src/first_contact

sort -u otp_wordlist.txt | grep -v '^$' > otp_wordlist.tmp
mv otp_wordlist.tmp otp_wordlist.txt

echo "Done: $(grep -c '' otp_wordlist.txt) unique words"