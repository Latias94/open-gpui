#include <algorithm>
#include <chrono>
#include <cstdlib>
#include <cstdio>
#include <vector>

#define IMGUI_DEFINE_MATH_OPERATORS
#include "imgui.h"
#include "imgui_internal.h"
#include "imgui.cpp"

using Clock = std::chrono::steady_clock;

__attribute__((noinline)) static int PreviewOnce(ImGuiWindow* host, ImGuiWindow* payload, float mouse_x)
{
    GImGui->IO.MousePos = ImVec2(mouse_x, 350.0f);
    ImGuiDockPreviewData data;
    ImGui::DockNodePreviewDockSetup(host, nullptr, payload, nullptr, &data, true, false);
    return int(data.IsDropAllowed) + int(data.IsCenterAvailable) * 2 + int(data.IsSidesAvailable) * 4
        + int(data.SplitDir) * 8;
}

static double MeasurePreviewSetup(ImGuiWindow* host, ImGuiWindow* payload, int iterations)
{
    constexpr int sample_count = 7;
    std::vector<double> samples;
    samples.reserve(sample_count);

    volatile int warmup_checksum = 0;
    for (int iteration = 0; iteration < 10000; ++iteration)
    {
        warmup_checksum ^= PreviewOnce(host, payload, 500.0f + float(iteration % 5));
    }

    volatile int checksum = warmup_checksum;
    for (int sample = 0; sample < sample_count; ++sample)
    {
        const auto started = Clock::now();
        for (int iteration = 0; iteration < iterations; ++iteration)
        {
            const int result = PreviewOnce(host, payload, 500.0f + float((iteration + sample) % 5));
            checksum ^= result;
            if ((result & 1) == 0)
                std::abort();
        }
        const auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - started).count();
        samples.push_back(double(elapsed) / double(iterations));
    }

    std::sort(samples.begin(), samples.end());
    if (checksum == 0x7fffffff)
        std::fprintf(stderr, "impossible checksum\n");
    return samples[samples.size() / 2];
}

int main()
{
    IMGUI_CHECKVERSION();
    ImGui::CreateContext();
    ImGuiIO& io = ImGui::GetIO();
    io.ConfigFlags |= ImGuiConfigFlags_DockingEnable;
    io.DisplaySize = ImVec2(1600.0f, 1000.0f);
    io.DeltaTime = 1.0f / 60.0f;
    io.IniFilename = nullptr;
    unsigned char* font_pixels = nullptr;
    int font_width = 0;
    int font_height = 0;
    int font_bytes_per_pixel = 0;
    io.Fonts->GetTexDataAsRGBA32(&font_pixels, &font_width, &font_height, &font_bytes_per_pixel);

    ImGui::NewFrame();
    ImGui::SetNextWindowPos(ImVec2(100.0f, 100.0f));
    ImGui::SetNextWindowSize(ImVec2(900.0f, 600.0f));
    ImGui::Begin("Benchmark Host");
    ImGuiWindow* host = ImGui::GetCurrentWindow();
    ImGui::End();

    ImGui::SetNextWindowPos(ImVec2(1100.0f, 100.0f));
    ImGui::SetNextWindowSize(ImVec2(420.0f, 260.0f));
    ImGui::Begin("Benchmark Payload");
    ImGuiWindow* payload = ImGui::GetCurrentWindow();
    ImGui::End();
    ImGui::Render();

    ImGui::NewFrame();
    ImGui::SetNextWindowPos(ImVec2(100.0f, 100.0f));
    ImGui::SetNextWindowSize(ImVec2(900.0f, 600.0f));
    ImGui::Begin("Benchmark Host");
    host = ImGui::GetCurrentWindow();
    payload = ImGui::FindWindowByName("Benchmark Payload");
    if (host == nullptr || payload == nullptr)
        return 2;
    const double ns_per_preview = MeasurePreviewSetup(host, payload, 1000000);
    ImGui::End();
    ImGui::Render();
    ImGui::DestroyContext();

    std::printf("{\"imgui_ns_per_preview\":%.6f}\n", ns_per_preview);
    return 0;
}
