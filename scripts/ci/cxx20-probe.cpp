#include <format>
#include <string>

// LadybugDB's generated C++ bridge includes <format>. Check the header,
// implementation and runtime before starting the full Rust dependency build.
int main() {
    return std::format("{} {}", "Crabot", 20) == "Crabot 20" ? 0 : 1;
}
