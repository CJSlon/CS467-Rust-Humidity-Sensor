# CS 467 - Rust Humidity Sensor

- [CS 467 - Rust Humidity Sensor](#cs-467---rust-humidity-sensor)
  - [Development Setup](#development-setup)
    - [Resources](#resources)
    - [Considerations](#considerations)
    - [Containerized Development](#containerized-development)
      - [Usage with this Project](#usage-with-this-project)

## Development Setup

### Resources

- https://github.com/raspberrypi/picotool
- https://github.com/raspberrypi/pico-sdk
- https://jamesachambers.com/getting-started-guide-raspberry-pi-pico/`
- https://code.visualstudio.com/docs/devcontainers/containers

### Considerations

Rust development for the Raspberry Pi Pico can be done on Windows, Linux, or macOS. In any case, the Rust compliler produces a `.elf` file that must be converted into a `.uf2` file to flash the Pico.

Developers are responsible for managing the tool chain and build dependencies on their systems. While cross-compilation is possible on various host operating systems, certain tool chain commands are not universal. This is okay so long as these commands can be bypassed or replaced with something equivalent. 

For example, `picotool` is helpful for abstracting loading and monitoring commands specific to the Pico. However, this requires the Pico to be visible to the host operating system as a USB device. If a developer uses a Linux container on a Windows or MacOS machine for building, the Pico won't be visible for direct loading unless COM ports are mapped between the host and container. Depending on the operating system this can require bypassing container-host hardware isolation constraints. As an alternative, `picotool` can be bypassed altogether by moving the loading step to the host machine. This is less convenient but allows for development flexibility.  

### Containerized Development

VSCode Dev Containers separate the development environment and the local host machine. This makes it possible to develop using tool chains without having to install or configure anything locally (other than VSCode and the official Dev Container extension).

From the Dev Container docs linked above:

> The Visual Studio Code Dev Containers extension lets you use a container as a full-featured development environment. It allows you to open any folder inside (or mounted into) a container and take advantage of Visual Studio Code's full feature set. A devcontainer.json file in your project tells VS Code how to access (or create) a development container with a well-defined tool and runtime stack. This container can be used to run an application or to separate tools, libraries, or runtimes needed for working with a codebase.
>
> Workspace files are mounted from the local file system or copied or cloned into the container. Extensions are installed and run inside the container, where they have full access to the tools, platform, and file system. This means that you can seamlessly switch your entire development environment just by connecting to a different container.
>
> This lets VS Code provide a local-quality development experience including full IntelliSense (completions), code navigation, and debugging regardless of where your tools (or code) are located.

As an aside, since Dev Containers run a VSCode server that communicates with the local VSCode's UI, it's also possible to develop with Dev Containers remotely, such as with a GitHub Codespace, or even on an embedded device!

The Dev Container defines everything required to bring up an equivalent environment on disparate hosts, and the lightweight spec files are tracked alongside the project in `git`. To get started, a new developer only needs to clone the repo, open the folder in VSCode, and launch the Dev Container.

#### Usage in this Project

The Dev Container spec is configured to install Debian Linux (v12) and Rust from an official Microsoft image. It then uses a `Dockerfile` to install the Linux system dependencies required for working with the Pico SDK and Rust. Though this is all that's truly needed for development, the spec is also able to install various nice-to-haves like `zsh` customization, `pico-sdk`, `picotool`, and `pico-examples` (and run their setup `make` steps).

Once running, the Dev Container can be treated as a full Linux machine, barring I/O device connections. If the host machine is also running Linux, mapping the USB COM port of the Pico to a USB port in the container is straightforward. This enables for the build chain to work end-to-end using `picotool`. If the host is MacOS or Windows, it's up to the developer to decide how to proceed. Pico flashing is as simple as dragging and dropping a `.uf2` into the Pico's root directory via File Exporer or Finder, so choosing to bypass `picotool` will not significantly affect the development experience. Additionally, if the Pico is not able to communicate with the container directly via USB, serial monitoring is still possible from a separate process on the host. VSCode also makes this easy with the official *Serial Monitor* extension.

While containerized development has many benefits, it is *not* required for development on this project. The only requirement is for each developer to be able to produce a valid `.uf2`, one way or another.

#### Sensor LED Patterns

The humidity sensor using LED sequences to indicate various status to the user.These statuses convey successful boot and various error states the sensor may encounter. The sensor immedialty attempts to initialize upon being pluged in and will render a humidity reading or error after indicateing a sucessful boot. The LED patterns and corrisponding meanings are outlined below:

| Pattern Discription                                       | Pattern Meaning                                     |
| --------------------------------------------------------- | --------------------------------------------------- |
| Series of illumnination bounding back and forth two times | Successful boot and initialization of the sensor    | 
| Series of three rapid repeated flashes of all LEDs        | Boot failure and unsucessful sensor initialization  |
| Single flashing **red** LED                               | Error requesting humidity read from sensor          |
| Single flashing **yellow** LED                            | Error recieving humidity read from sensor           |
| Single flashing **green** LED                             | Sensor is busy with existing request                |

Upon a successfuly sensor read the sensor will illuminate all LEDs up to and including the LED for the bracked ranges outlined below

| LED Number  | LED Color | RH%       | Description         |
| :---------: | --------- | :---:     | -----------         |
| 1           | Red       |  < 20%    | Critically Dry      |
| 2           | Yellow    | 20 - 40%  | Dry                 |
| 3           | Green     | 40 - 50%  | Comfortable         |
| 4           | Green     | 50 - 60%  | Comfortable         |
| 5           | Yellow    | 60 - 70%  | Humid               |
| 6           | Red       | >70%      | Critically Humid    |

