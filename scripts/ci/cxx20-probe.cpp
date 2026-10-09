#include <atomic>
#include <format>
#include <string>

// Exercise both the generated bridge's formatting and the C API's atomic_ref.
// Xcode 15.4 supports <format> but cannot compile the latter.
int main() {
    int value = 20;
    alignas(std::atomic_ref<void*>::required_alignment) void* connection = &value;
    std::atomic_ref<void*> atomic_connection(connection);
    if (atomic_connection.exchange(nullptr) != &value || atomic_connection.load() != nullptr) {
        return 1;
    }
    return std::format("{} {}", "Crabot", 20) == "Crabot 20" ? 0 : 1;
}
