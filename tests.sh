#!/bin/bash
cd "$(dirname "$0")"

test=1

for i in $(ls example); do
  target/release/bengal example/$i > /dev/null || { test=0; echo -e "\e[31m$i finished with error\e[0m"; }
done

if [ $test == 0 ]; then
  trap 'echo "Some tests failed" > &2' ERR
fi