#include <algorithm>
#include <chrono>
#include <cstring>
#include <cstdlib>
#include <cstdio>
#include <vector>

#define IMGUI_DEFINE_MATH_OPERATORS
#include "imgui.h"
#include "imgui_internal.h"
#include "imgui.cpp"

using Clock = std::chrono::steady_clock;

[[noreturn]] static void BenchmarkFail(const char* message)
{
    std::fprintf(stderr, "imgui docking benchmark failed: %s\n", message);
    std::abort();
}

__attribute__((noinline)) static int PreviewOnce(ImGuiWindow* host, ImGuiWindow* payload, float mouse_x)
{
    GImGui->IO.MousePos = ImVec2(mouse_x, 350.0f);
    ImGuiDockPreviewData data;
    ImGui::DockNodePreviewDockSetup(host, nullptr, payload, nullptr, &data, true, false);
    return int(data.IsDropAllowed) + int(data.IsCenterAvailable) * 2 + int(data.IsSidesAvailable) * 4
        + int(data.SplitDir) * 8;
}

static void ResetPreviewDrawLists(ImGuiWindow* host, ImGuiWindow* payload)
{
    ImGui::GetForegroundDrawList(host->Viewport)->_ResetForNewFrame();
    if (payload->Viewport != host->Viewport)
        ImGui::GetForegroundDrawList(payload->Viewport)->_ResetForNewFrame();
}

__attribute__((noinline)) static int DockableTargetOnce(ImGuiWindow* host, ImGuiWindow* payload, float mouse_x)
{
    ImGuiContext& g = *GImGui;
    ImGuiWindow* payload_window = payload;
    ImGuiWindow* previous_current_window = g.CurrentWindow;
    ImGuiWindow* previous_hovered_window = g.HoveredWindow;
    ImGuiWindow* previous_hovered_window_under_moving_window = g.HoveredWindowUnderMovingWindow;
    const bool previous_mouse_down = g.IO.MouseDown[0];

    g.CurrentWindow = host;
    g.HoveredWindow = host;
    g.HoveredWindowUnderMovingWindow = host;
    g.IO.MousePos = ImVec2(
        host->Pos.x + 20.0f + float(int(mouse_x) % 5),
        host->Pos.y + 10.0f);
    g.IO.MouseDown[0] = true;
    g.DragDropActive = true;
    g.DragDropWithinSource = false;
    g.DragDropWithinTarget = false;
    g.DragDropSourceFlags = ImGuiDragDropFlags_None;
    g.DragDropSourceFrameCount = g.FrameCount;
    g.DragDropMouseButton = 0;
    g.DragDropAcceptFlagsCurr = ImGuiDragDropFlags_None;
    g.DragDropAcceptIdCurr = 0;
    g.DragDropAcceptIdPrev = host->ID;
    g.DragDropAcceptIdCurrRectSurface = FLT_MAX;
    g.DragDropAcceptFrameCount = g.FrameCount - 1;
    g.DragDropPayload.Clear();
    g.DragDropPayload.SourceId = payload->ID;
    g.DragDropPayload.SourceParentId = 0;
    g.DragDropPayload.Data = &payload_window;
    g.DragDropPayload.DataSize = sizeof(payload_window);
    g.DragDropPayload.DataFrameCount = g.FrameCount;
    ImStrncpy(
        g.DragDropPayload.DataType,
        IMGUI_PAYLOAD_TYPE_WINDOW,
        IM_ARRAYSIZE(g.DragDropPayload.DataType));

    ImGui::BeginDockableDragDropTarget(host);
    const int result = int(g.DragDropAcceptIdCurr == host->ID) + int(g.DragDropPayload.Preview) * 2
        + int(g.DragDropWithinTarget == false) * 4;

    g.IO.MouseDown[0] = previous_mouse_down;
    g.CurrentWindow = previous_current_window;
    g.HoveredWindow = previous_hovered_window;
    g.HoveredWindowUnderMovingWindow = previous_hovered_window_under_moving_window;
    ImGui::ClearDragDrop();
    return result;
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

static double MeasureDockableTarget(ImGuiWindow* host, ImGuiWindow* payload, int iterations)
{
    constexpr int sample_count = 7;
    std::vector<double> samples;
    samples.reserve(sample_count);

    volatile int warmup_checksum = 0;
    for (int iteration = 0; iteration < 5000; ++iteration)
    {
        if ((iteration & 255) == 0)
            ResetPreviewDrawLists(host, payload);
        warmup_checksum ^= DockableTargetOnce(host, payload, 500.0f + float(iteration % 5));
    }
    ResetPreviewDrawLists(host, payload);

    volatile int checksum = warmup_checksum;
    for (int sample = 0; sample < sample_count; ++sample)
    {
        const auto started = Clock::now();
        for (int iteration = 0; iteration < iterations; ++iteration)
        {
            // Keep one-frame overlay output bounded while measuring accepted previews.
            if ((iteration & 255) == 0)
                ResetPreviewDrawLists(host, payload);
            const int result = DockableTargetOnce(host, payload, 500.0f + float((iteration + sample) % 5));
            checksum ^= result;
            if ((result & 3) != 3)
            {
                std::fprintf(
                    stderr,
                    "dockable target benchmark failed: result=%d sample=%d iteration=%d\n",
                    result,
                    sample,
                    iteration);
                std::abort();
            }
        }
        const auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - started).count();
        samples.push_back(double(elapsed) / double(iterations));
        ResetPreviewDrawLists(host, payload);
    }

    std::sort(samples.begin(), samples.end());
    if (checksum == 0x7fffffff)
        std::fprintf(stderr, "impossible checksum\n");
    return samples[samples.size() / 2];
}

static double MeasureKnownHostPreviewSetup(const std::vector<ImGuiWindow*>& hosts, ImGuiWindow* payload, int iterations)
{
    constexpr int sample_count = 7;
    std::vector<double> samples;
    samples.reserve(sample_count);

    volatile int warmup_checksum = 0;
    for (int iteration = 0; iteration < 10000; ++iteration)
    {
        ImGuiWindow* host = hosts[size_t(iteration) % hosts.size()];
        warmup_checksum ^= PreviewOnce(host, payload, 500.0f + float(iteration % 5));
    }

    volatile int checksum = warmup_checksum;
    for (int sample = 0; sample < sample_count; ++sample)
    {
        const auto started = Clock::now();
        for (int iteration = 0; iteration < iterations; ++iteration)
        {
            ImGuiWindow* host = hosts[size_t(iteration + sample) % hosts.size()];
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

static double MeasureKnownHostDockableTarget(const std::vector<ImGuiWindow*>& hosts, ImGuiWindow* payload, int iterations)
{
    constexpr int sample_count = 7;
    std::vector<double> samples;
    samples.reserve(sample_count);

    volatile int warmup_checksum = 0;
    for (int iteration = 0; iteration < 5000; ++iteration)
    {
        ImGuiWindow* host = hosts[size_t(iteration) % hosts.size()];
        if ((iteration & 255) == 0)
            ResetPreviewDrawLists(host, payload);
        warmup_checksum ^= DockableTargetOnce(host, payload, 500.0f + float(iteration % 5));
    }
    ResetPreviewDrawLists(hosts.front(), payload);

    volatile int checksum = warmup_checksum;
    for (int sample = 0; sample < sample_count; ++sample)
    {
        const auto started = Clock::now();
        for (int iteration = 0; iteration < iterations; ++iteration)
        {
            ImGuiWindow* host = hosts[size_t(iteration + sample) % hosts.size()];
            if ((iteration & 255) == 0)
                ResetPreviewDrawLists(host, payload);
            const int result = DockableTargetOnce(host, payload, 500.0f + float((iteration + sample) % 5));
            checksum ^= result;
            if ((result & 3) != 3)
            {
                std::fprintf(
                    stderr,
                    "dockable target multiview benchmark failed: result=%d sample=%d iteration=%d\n",
                    result,
                    sample,
                    iteration);
                std::abort();
            }
        }
        const auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - started).count();
        samples.push_back(double(elapsed) / double(iterations));
        ResetPreviewDrawLists(hosts.front(), payload);
    }

    std::sort(samples.begin(), samples.end());
    if (checksum == 0x7fffffff)
        std::fprintf(stderr, "impossible checksum\n");
    return samples[samples.size() / 2];
}

struct RehomeItemSpec
{
    const char* Id;
    const char* WindowName;
};

enum RehomeNodeKind
{
    RehomeNode_Tabs,
    RehomeNode_Split,
};

struct RehomeTopologyNode
{
    RehomeNodeKind Kind;
    ImGuiAxis SplitAxis;
    float SecondChildFraction;
    int FirstItem;
    int ItemCount;
    int SelectedItem;
    int FirstChild;
    int ChildCount;
};

struct RehomeSpaceSpec
{
    const char* Id;
    const char* DockspaceLabel;
    int RootNode;
};

struct RehomeStep
{
    const char* Id;
    int Payload;
    int SourceSpace;
    int Target;
    int TargetSpace;
    ImGuiDir SplitDir;
    const char* Zone;
};

struct RehomeCheckpointSpec
{
    const char* AfterStep;
    int FirstRoot;
    int RootCount;
};

#include "three_dockspace_rehome.generated.h"

struct ThreeDockspaceRehomeFixture
{
    ImGuiID Roots[IM_ARRAYSIZE(RehomeSpaces)];
    ImGuiWindow* Windows[IM_ARRAYSIZE(RehomeItems)];
};

static void CheckRehomeState(bool condition, const char* message, bool& valid)
{
    if (!condition)
    {
        std::fprintf(stderr, "rehome state mismatch: %s\n", message);
        valid = false;
    }
}

static bool IsExactDockspaceRoot(ImGuiDockNode* node, ImGuiID expected_root)
{
    return node != nullptr && node->ID == expected_root && node->ParentNode == nullptr;
}

static void ValidateExactDockspaceRootPredicate()
{
    ImGuiDockNode expected_root(1);
    ImGuiDockNode extra_root(2);

    if (!IsExactDockspaceRoot(&expected_root, expected_root.ID))
        BenchmarkFail("exact dockspace root predicate rejected the root");

    extra_root.ChildNodes[0] = &expected_root;
    expected_root.ParentNode = &extra_root;
    if (IsExactDockspaceRoot(&expected_root, expected_root.ID))
        BenchmarkFail("exact dockspace root predicate accepted an extra root wrapper");

    expected_root.ParentNode = nullptr;
    extra_root.ChildNodes[0] = nullptr;
}

static ImGuiDockNode* ValidateRehomeTopologyNode(
    const ThreeDockspaceRehomeFixture& fixture,
    int node_index,
    bool& valid)
{
    const RehomeTopologyNode& spec = RehomeNodes[node_index];
    if (spec.Kind == RehomeNode_Tabs)
    {
        ImGuiWindow* first = fixture.Windows[RehomeNodeItems[spec.FirstItem]];
        ImGuiDockNode* tabs = first != nullptr ? first->DockNode : nullptr;
        CheckRehomeState(tabs != nullptr, "tabs node ownership", valid);
        if (tabs == nullptr)
            return nullptr;

        CheckRehomeState(tabs->Windows.Size == spec.ItemCount, "tabs window count", valid);
        for (int index = 0; index < spec.ItemCount; ++index)
        {
            ImGuiWindow* window = fixture.Windows[RehomeNodeItems[spec.FirstItem + index]];
            CheckRehomeState(
                window != nullptr && window->DockNode == tabs,
                "tabs item ownership",
                valid);
        }
        if (spec.ItemCount > 1)
        {
            CheckRehomeState(
                tabs->TabBar != nullptr && tabs->TabBar->Tabs.Size == spec.ItemCount,
                "tab bar item count",
                valid);
            if (tabs->TabBar != nullptr && tabs->TabBar->Tabs.Size == spec.ItemCount)
            {
                for (int index = 0; index < spec.ItemCount; ++index)
                {
                    ImGuiWindow* expected =
                        fixture.Windows[RehomeNodeItems[spec.FirstItem + index]];
                    CheckRehomeState(
                        tabs->TabBar->Tabs[index].Window == expected,
                        "tab order",
                        valid);
                }
                const ImGuiID selected_tab = tabs->TabBar->NextSelectedTabId != 0
                    ? tabs->TabBar->NextSelectedTabId
                    : tabs->TabBar->SelectedTabId;
                CheckRehomeState(
                    selected_tab == fixture.Windows[spec.SelectedItem]->TabId,
                    "selected tab",
                    valid);
            }
        }
        return tabs;
    }

    CheckRehomeState(spec.ChildCount == 2, "split child count", valid);
    if (spec.ChildCount != 2)
        return nullptr;
    ImGuiDockNode* first = ValidateRehomeTopologyNode(
        fixture,
        RehomeNodeChildren[spec.FirstChild],
        valid);
    ImGuiDockNode* second = ValidateRehomeTopologyNode(
        fixture,
        RehomeNodeChildren[spec.FirstChild + 1],
        valid);
    ImGuiDockNode* split = first != nullptr ? first->ParentNode : nullptr;
    CheckRehomeState(
        split != nullptr && second != nullptr && second->ParentNode == split,
        "split child ownership",
        valid);
    if (split == nullptr || second == nullptr || second->ParentNode != split)
        return split;
    CheckRehomeState(split->IsSplitNode(), "split node kind", valid);
    CheckRehomeState(split->SplitAxis == spec.SplitAxis, "split axis", valid);
    CheckRehomeState(split->ChildNodes[0] == first, "first split child order", valid);
    CheckRehomeState(split->ChildNodes[1] == second, "second split child order", valid);
    const float first_extent = spec.SplitAxis == ImGuiAxis_X ? first->SizeRef.x : first->SizeRef.y;
    const float second_extent = spec.SplitAxis == ImGuiAxis_X ? second->SizeRef.x : second->SizeRef.y;
    const float total_extent = first_extent + second_extent;
    const float actual_second_fraction = total_extent > 0.0f ? second_extent / total_extent : -1.0f;
    float fraction_delta = actual_second_fraction - spec.SecondChildFraction;
    if (fraction_delta < 0.0f)
        fraction_delta = -fraction_delta;
    CheckRehomeState(fraction_delta <= 0.02f, "split fraction", valid);
    return split;
}

static bool ContainsDockNode(
    const std::vector<ImGuiDockNode*>& nodes,
    ImGuiDockNode* expected)
{
    return std::find(nodes.begin(), nodes.end(), expected) != nodes.end();
}

static void CollectReachableDockNodes(
    ImGuiDockNode* node,
    std::vector<ImGuiDockNode*>& reachable,
    bool& valid)
{
    CheckRehomeState(node != nullptr, "reachable dock node", valid);
    if (node == nullptr)
        return;
    if (ContainsDockNode(reachable, node))
    {
        CheckRehomeState(false, "dock node cycle or shared child", valid);
        return;
    }
    reachable.push_back(node);
    CheckRehomeState(
        ImGui::DockContextFindNodeByID(GImGui, node->ID) == node,
        "reachable node context identity",
        valid);
    for (ImGuiDockNode* child : node->ChildNodes)
    {
        if (child == nullptr)
            continue;
        CheckRehomeState(child->ParentNode == node, "dock child parent identity", valid);
        CollectReachableDockNodes(child, reachable, valid);
    }
}

static bool IsLiveDockNode(const ImGuiDockNode* node)
{
    return node != nullptr;
}

static void ValidateRehomeRuntimeConsistency(
    const ThreeDockspaceRehomeFixture& fixture,
    bool& valid)
{
    for (int index = 0; index < IM_ARRAYSIZE(RehomeItems); ++index)
    {
        ImGuiWindow* window = fixture.Windows[index];
        CheckRehomeState(window != nullptr, "rehome window lookup", valid);
        if (window == nullptr)
            continue;
        CheckRehomeState(window->DockNode != nullptr, "rehome window dock node", valid);
        if (window->DockNode == nullptr)
            continue;
        CheckRehomeState(
            window->DockId == window->DockNode->ID,
            "rehome window DockId/DockNode identity",
            valid);
        CheckRehomeState(
            ImGui::DockContextFindNodeByID(GImGui, window->DockId) == window->DockNode,
            "rehome window dock context identity",
            valid);
    }

    std::vector<ImGuiDockNode*> reachable;
    reachable.reserve(GImGui->DockContext.Nodes.Data.Size);
    for (int index = 0; index < IM_ARRAYSIZE(RehomeSpaces); ++index)
    {
        const std::size_t first_reachable = reachable.size();
        ImGuiDockNode* root = ImGui::DockContextFindNodeByID(GImGui, fixture.Roots[index]);
        CollectReachableDockNodes(root, reachable, valid);
        CheckRehomeState(
            root != nullptr
                && (root->CentralNode == nullptr
                    || std::find(
                           reachable.begin() + first_reachable,
                           reachable.end(),
                           root->CentralNode)
                        != reachable.end()),
            "dockspace central node reachability",
            valid);
    }

    int live_node_count = 0;
    for (const ImGuiStoragePair& pair : GImGui->DockContext.Nodes.Data)
    {
        ImGuiDockNode* node = static_cast<ImGuiDockNode*>(pair.val_p);
        if (!IsLiveDockNode(node))
            continue;
        live_node_count++;
        CheckRehomeState(pair.key == node->ID, "dock context node key identity", valid);
        CheckRehomeState(
            ContainsDockNode(reachable, node),
            "orphan live dock node",
            valid);
    }
    CheckRehomeState(
        live_node_count == static_cast<int>(reachable.size()),
        "reachable/live dock node set equality",
        valid);
}

static bool ValidateThreeDockspaceRehomeTopology(
    const ThreeDockspaceRehomeFixture& fixture,
    const int* root_nodes,
    int root_count)
{
    bool valid = true;
    CheckRehomeState(
        root_count == IM_ARRAYSIZE(RehomeSpaces),
        "checkpoint dockspace root count",
        valid);
    if (root_count != IM_ARRAYSIZE(RehomeSpaces))
        return false;
    for (int index = 0; index < IM_ARRAYSIZE(RehomeSpaces); ++index)
    {
        ImGuiDockNode* root = ValidateRehomeTopologyNode(
            fixture,
            root_nodes[index],
            valid);
        CheckRehomeState(
            IsExactDockspaceRoot(root, fixture.Roots[index]),
            "dockspace root ownership",
            valid);
    }
    ValidateRehomeRuntimeConsistency(fixture, valid);
    return valid;
}

static bool ValidateThreeDockspaceRehomeFixture(const ThreeDockspaceRehomeFixture& fixture)
{
    int initial_roots[IM_ARRAYSIZE(RehomeSpaces)];
    for (int index = 0; index < IM_ARRAYSIZE(RehomeSpaces); ++index)
        initial_roots[index] = RehomeSpaces[index].RootNode;
    return ValidateThreeDockspaceRehomeTopology(
        fixture,
        initial_roots,
        IM_ARRAYSIZE(initial_roots));
}

static void BuildRehomeTopologyNode(int node_index, ImGuiID dock_node)
{
    const RehomeTopologyNode& spec = RehomeNodes[node_index];
    if (spec.Kind == RehomeNode_Tabs)
    {
        for (int index = 0; index < spec.ItemCount; ++index)
        {
            ImGui::DockBuilderDockWindow(
                RehomeItems[RehomeNodeItems[spec.FirstItem + index]].WindowName,
                dock_node);
        }
        return;
    }

    if (spec.ChildCount != 2)
        BenchmarkFail("generated rehome split must have two children");
    ImGuiID first = 0;
    ImGuiID second = 0;
    const ImGuiDir direction = spec.SplitAxis == ImGuiAxis_X ? ImGuiDir_Right : ImGuiDir_Down;
    ImGui::DockBuilderSplitNode(
        dock_node,
        direction,
        spec.SecondChildFraction,
        &second,
        &first);
    BuildRehomeTopologyNode(RehomeNodeChildren[spec.FirstChild], first);
    BuildRehomeTopologyNode(RehomeNodeChildren[spec.FirstChild + 1], second);
}

static ThreeDockspaceRehomeFixture BuildThreeDockspaceRehomeFixture()
{
    ThreeDockspaceRehomeFixture fixture = {};
    for (int index = 0; index < IM_ARRAYSIZE(RehomeSpaces); ++index)
    {
        fixture.Roots[index] = ImHashStr(RehomeSpaces[index].DockspaceLabel);
        ImGui::DockBuilderAddNode(fixture.Roots[index], ImGuiDockNodeFlags_DockSpace);
        ImGui::DockBuilderSetNodeSize(fixture.Roots[index], ImVec2(480.0f, 320.0f));
        ImGui::DockBuilderSetNodePos(
            fixture.Roots[index],
            ImVec2(float(index) * 500.0f, 0.0f));
        BuildRehomeTopologyNode(RehomeSpaces[index].RootNode, fixture.Roots[index]);
        ImGui::DockBuilderFinish(fixture.Roots[index]);
    }

    for (int index = 0; index < IM_ARRAYSIZE(RehomeItems); ++index)
    {
        fixture.Windows[index] = ImGui::FindWindowByName(RehomeItems[index].WindowName);
        if (fixture.Windows[index] == nullptr)
            BenchmarkFail("cycle window lookup failed");
    }
    return fixture;
}

static void ApplyRehomeTopologySelections(
    const ThreeDockspaceRehomeFixture& fixture,
    int node_index)
{
    const RehomeTopologyNode& spec = RehomeNodes[node_index];
    if (spec.Kind == RehomeNode_Tabs)
    {
        if (spec.ItemCount <= 1)
            return;
        ImGuiWindow* selected = fixture.Windows[spec.SelectedItem];
        ImGuiDockNode* tabs = selected != nullptr ? selected->DockNode : nullptr;
        if (tabs == nullptr || tabs->TabBar == nullptr)
            BenchmarkFail("generated selected tab has no tab bar");
        tabs->SelectedTabId = selected->TabId;
        tabs->TabBar->SelectedTabId = selected->TabId;
        tabs->TabBar->NextSelectedTabId = 0;
        tabs->TabBar->VisibleTabId = selected->TabId;
        return;
    }
    for (int index = 0; index < spec.ChildCount; ++index)
        ApplyRehomeTopologySelections(
            fixture,
            RehomeNodeChildren[spec.FirstChild + index]);
}

static void ApplyRehomeSelections(const ThreeDockspaceRehomeFixture& fixture)
{
    for (const RehomeSpaceSpec& space : RehomeSpaces)
        ApplyRehomeTopologySelections(fixture, space.RootNode);
}

static ImGuiID DockspaceRootForSpace(
    const ThreeDockspaceRehomeFixture& fixture,
    int space)
{
    if (space < 0 || space >= IM_ARRAYSIZE(RehomeSpaces))
        BenchmarkFail("unknown logical dockspace");
    return fixture.Roots[space];
}

static void RefreshRehomeDockspaceMetadata(
    const ThreeDockspaceRehomeFixture& fixture)
{
    for (ImGuiID root_id : fixture.Roots)
    {
        ImGuiDockNode* root = ImGui::DockContextFindNodeByID(GImGui, root_id);
        if (root == nullptr)
            BenchmarkFail("rehome dockspace root disappeared");
        ImGui::DockNodeUpdateForRootNode(root);
    }
}

static void CommitDockWindow(
    const ThreeDockspaceRehomeFixture& fixture,
    const RehomeStep& step)
{
    ImGuiWindow* payload = ImGui::FindWindowByName(RehomeItems[step.Payload].WindowName);
    ImGuiWindow* target = ImGui::FindWindowByName(RehomeItems[step.Target].WindowName);
    if (payload == nullptr || target == nullptr || payload->DockNode == nullptr
        || target->DockNode == nullptr
        || ImGui::DockNodeGetRootNode(payload->DockNode)->ID != DockspaceRootForSpace(fixture, step.SourceSpace)
        || ImGui::DockNodeGetRootNode(target->DockNode)->ID != DockspaceRootForSpace(fixture, step.TargetSpace))
    {
        std::fprintf(
            stderr,
            "invalid rehome step: %s@%s>%s@%s:%s\n",
            RehomeItems[step.Payload].Id,
            RehomeSpaces[step.SourceSpace].Id,
            RehomeItems[step.Target].Id,
            RehomeSpaces[step.TargetSpace].Id,
            step.Zone);
        BenchmarkFail("named-item source or target lookup failed");
    }
    ImGuiDockRequest request;
    request.Type = ImGuiDockRequestType_Dock;
    request.DockTargetWindow = nullptr;
    request.DockTargetNode = target->DockNode;
    request.DockPayload = payload;
    request.DockSplitDir = step.SplitDir;
    request.DockSplitRatio = 0.5f;
    request.DockSplitOuter = false;
    ImGui::DockContextProcessDock(GImGui, &request);
    RefreshRehomeDockspaceMetadata(fixture);
}

static void AuditThreeDockspaceRehomeSequence(
    const ThreeDockspaceRehomeFixture& fixture)
{
    if (IM_ARRAYSIZE(RehomeSteps) != IM_ARRAYSIZE(RehomeCheckpoints))
        BenchmarkFail("generated rehome step/checkpoint count mismatch");
    for (int index = 0; index < IM_ARRAYSIZE(RehomeSteps); ++index)
    {
        const RehomeStep& step = RehomeSteps[index];
        const RehomeCheckpointSpec& checkpoint = RehomeCheckpoints[index];
        if (std::strcmp(step.Id, checkpoint.AfterStep) != 0)
            BenchmarkFail("generated rehome checkpoint order mismatch");
        CommitDockWindow(fixture, step);
        if (!ValidateThreeDockspaceRehomeTopology(
                fixture,
                RehomeCheckpointRoots + checkpoint.FirstRoot,
                checkpoint.RootCount))
        {
            std::fprintf(stderr, "rehome checkpoint failed after step: %s\n", step.Id);
            BenchmarkFail("three-dockspace checkpoint validation failed");
        }
    }
    if (!ValidateThreeDockspaceRehomeFixture(fixture))
        BenchmarkFail("audited rehome sequence did not restore the initial topology");
}

__attribute__((noinline)) static void ApplyThreeDockspaceRehomeSequence(
    const ThreeDockspaceRehomeFixture& fixture)
{
    for (const RehomeStep& step : RehomeSteps)
        CommitDockWindow(fixture, step);
}

static double MeasureThreeDockspaceRehomeSequence(
    const ThreeDockspaceRehomeFixture& fixture,
    int iterations)
{
    constexpr int sample_count = 7;
    std::vector<double> samples;
    samples.reserve(sample_count);

    for (int iteration = 0; iteration < std::min(iterations, 2000); ++iteration)
        ApplyThreeDockspaceRehomeSequence(fixture);
    if (!ValidateThreeDockspaceRehomeFixture(fixture))
        BenchmarkFail("warmup did not restore the initial topology");

    volatile int checksum = 0;
    for (int sample = 0; sample < sample_count; ++sample)
    {
        if (!ValidateThreeDockspaceRehomeFixture(fixture))
            BenchmarkFail("sample did not start from the initial topology");
        const auto started = Clock::now();
        for (int iteration = 0; iteration < iterations; ++iteration)
        {
            ApplyThreeDockspaceRehomeSequence(fixture);
            checksum ^= IM_ARRAYSIZE(RehomeSteps);
        }
        const auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - started).count();
        if (!ValidateThreeDockspaceRehomeFixture(fixture))
            BenchmarkFail("sample did not restore the initial topology");
        samples.push_back(double(elapsed) / double(iterations));
    }

    std::sort(samples.begin(), samples.end());
    if (checksum == 0x7fffffff)
        std::fprintf(stderr, "impossible checksum\n");
    return samples[samples.size() / 2];
}

static void BeginBenchmarkHost(const char* name, float x, float y, float width, float height)
{
    ImGui::SetNextWindowPos(ImVec2(x, y));
    ImGui::SetNextWindowSize(ImVec2(width, height));
    ImGui::Begin(name);
    ImGui::End();
}

int main()
{
    IMGUI_CHECKVERSION();
    ImGui::CreateContext();
    ImGuiIO& io = ImGui::GetIO();
    io.ConfigFlags |= ImGuiConfigFlags_DockingEnable;
    io.DisplaySize = ImVec2(1600.0f, 1400.0f);
    io.DeltaTime = 1.0f / 60.0f;
    io.IniFilename = nullptr;
    unsigned char* font_pixels = nullptr;
    int font_width = 0;
    int font_height = 0;
    int font_bytes_per_pixel = 0;
    io.Fonts->GetTexDataAsRGBA32(&font_pixels, &font_width, &font_height, &font_bytes_per_pixel);

    constexpr int multiview_host_count = 8;

    ImGui::NewFrame();
    BeginBenchmarkHost("Benchmark Host", 100.0f, 100.0f, 900.0f, 600.0f);
    for (int index = 0; index < multiview_host_count; ++index)
    {
        char name[64];
        std::snprintf(name, sizeof(name), "Benchmark Multi Host %d", index);
        BeginBenchmarkHost(
            name,
            100.0f + float(index % 4) * 360.0f,
            760.0f + float(index / 4) * 300.0f,
            320.0f,
            240.0f);
    }

    ImGui::SetNextWindowPos(ImVec2(1100.0f, 100.0f));
    ImGui::SetNextWindowSize(ImVec2(420.0f, 260.0f));
    ImGui::Begin("Benchmark Payload");
    ImGuiWindow* payload = ImGui::GetCurrentWindow();
    ImGui::End();
    for (const RehomeItemSpec& item : RehomeItems)
        BeginBenchmarkHost(item.WindowName, 0.0f, 0.0f, 320.0f, 220.0f);
    ImGui::Render();

    ImGui::NewFrame();
    ValidateExactDockspaceRootPredicate();
    const ThreeDockspaceRehomeFixture rehome_fixture = BuildThreeDockspaceRehomeFixture();
    ImGui::SetNextWindowPos(ImVec2(100.0f, 100.0f));
    ImGui::SetNextWindowSize(ImVec2(900.0f, 600.0f));
    ImGui::Begin("Benchmark Host");
    ImGuiWindow* host = ImGui::GetCurrentWindow();
    ImGui::End();

    std::vector<ImGuiWindow*> multiview_hosts;
    multiview_hosts.reserve(multiview_host_count);
    for (int index = 0; index < multiview_host_count; ++index)
    {
        char name[64];
        std::snprintf(name, sizeof(name), "Benchmark Multi Host %d", index);
        ImGui::SetNextWindowPos(ImVec2(100.0f + float(index % 4) * 360.0f, 760.0f + float(index / 4) * 300.0f));
        ImGui::SetNextWindowSize(ImVec2(320.0f, 240.0f));
        ImGui::Begin(name);
        multiview_hosts.push_back(ImGui::GetCurrentWindow());
        ImGui::End();
    }

    ImGui::SetNextWindowPos(ImVec2(1100.0f, 100.0f));
    ImGui::SetNextWindowSize(ImVec2(420.0f, 260.0f));
    ImGui::Begin("Benchmark Payload");
    payload = ImGui::GetCurrentWindow();
    ImGui::End();
    for (const RehomeItemSpec& item : RehomeItems)
    {
        ImGui::Begin(item.WindowName);
        ImGui::End();
    }
    ApplyRehomeSelections(rehome_fixture);

    if (host == nullptr || payload == nullptr || multiview_hosts.size() != multiview_host_count)
        return 2;
    if (!ValidateThreeDockspaceRehomeFixture(rehome_fixture))
        BenchmarkFail("initial three-dockspace topology validation failed");
    AuditThreeDockspaceRehomeSequence(rehome_fixture);
    const double ns_per_preview = MeasurePreviewSetup(host, payload, 1000000);
    const double multiview_known_host_ns_per_preview =
        MeasureKnownHostPreviewSetup(multiview_hosts, payload, 1000000);
    const double dockable_target_ns_per_preview = MeasureDockableTarget(host, payload, 50000);
    const double multiview_dockable_target_known_host_ns_per_preview =
        MeasureKnownHostDockableTarget(multiview_hosts, payload, 50000);
    const double three_dockspace_rehome_cycle_ns_per_sequence =
        MeasureThreeDockspaceRehomeSequence(rehome_fixture, 2000);
    ImGui::Render();
    ImGui::DestroyContext();

    std::printf(
        "{\"imgui_ns_per_preview\":%.6f,"
        "\"imgui_multiview_known_host_ns_per_preview\":%.6f,"
        "\"imgui_dockable_target_ns_per_preview\":%.6f,"
        "\"imgui_multiview_dockable_target_known_host_ns_per_preview\":%.6f,"
        "\"imgui_three_dockspace_rehome_cycle_ns_per_sequence\":%.6f,"
        "\"imgui_three_dockspace_rehome_operation_step_count\":%d,"
        "\"imgui_three_dockspace_rehome_workload_contract\":\"%s\"}\n",
        ns_per_preview,
        multiview_known_host_ns_per_preview,
        dockable_target_ns_per_preview,
        multiview_dockable_target_known_host_ns_per_preview,
        three_dockspace_rehome_cycle_ns_per_sequence,
        IM_ARRAYSIZE(RehomeSteps),
        RehomeWorkloadContract);
    return 0;
}
