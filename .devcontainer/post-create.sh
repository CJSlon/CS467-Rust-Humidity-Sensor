
# Following the instructions in this guide:
#    https://jamesachambers.com/getting-started-guide-raspberry-pi-pico/
# Thanks Colin for finding it!

git clone https://github.com/raspberrypi/pico-sdk.git
cd pico-sdk
git submodule update --init

cd ..
git clone https://github.com/raspberrypi/picotool.git
cd picotool
mkdir build
cd build
cmake -DPICO_SDK_PATH=../../pico-sdk ..
make
# The guide makes a note of doing this but it's not a requirement
# This essentially just places the binary where the system
# can find it so we don't have to specify the whole path each
# time we call picotool
sudo make install

# Get the official examples and prepare makefiles, but don't actually
# compile
cd ../..
git clone https://github.com/raspberrypi/pico-examples
cd pico-examples
mkdir build
cd build
cmake -DPICO_SDK_PATH=../../pico-sdk -DPICO_BOARD=pico ..

echo "Done"
