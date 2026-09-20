let selectedNode = null;
let historyData = [];


async function api(url) {

    const response = await fetch(url);

    if (!response.ok) {
        throw new Error(
            `API request failed: ${response.status}`
        );
    }

    return response.json();
}


/*
    Load all nodes from:

    GET /api/nodes
*/

async function loadNodes() {

    try {

        setConnection(true);

        const nodes = await api(
            "/api/nodes"
        );

        renderNodes(nodes);

        updateOverview(nodes);

    } catch (error) {

        console.error(error);

        setConnection(false);

    }
}


/*
    Update total/online/offline counters.
*/

function updateOverview(nodes) {

    const online =
        nodes.filter(
            node => node.status === "online"
        ).length;

    const offline =
        nodes.filter(
            node => node.status === "offline"
        ).length;


    document.getElementById(
        "totalNodes"
    ).textContent = nodes.length;


    document.getElementById(
        "onlineNodes"
    ).textContent = online;


    document.getElementById(
        "offlineNodes"
    ).textContent = offline;
}


/*
    Render node cards.
*/

function renderNodes(nodes) {

    const container =
        document.getElementById(
            "nodesContainer"
        );


    if (nodes.length === 0) {

        container.innerHTML = `
            <div class="loading">
                No CloudMesh nodes registered.
            </div>
        `;

        return;
    }


    container.innerHTML =
        nodes.map(node => `

            <div
                class="node-card"
                onclick="selectNode('${escapeHtml(node.node_id)}')"
            >

                <div class="node-header">

                    <div class="node-name">
                        ${escapeHtml(node.node_id)}
                    </div>

                    <div
                        class="node-status ${node.status}"
                    >

                        <span
                            class="node-status-dot"
                        ></span>

                        ${node.status}

                    </div>

                </div>


                <div class="node-info">

                    <div class="node-info-row">
                        <span>Hostname</span>
                        <strong>
                            ${escapeHtml(node.hostname)}
                        </strong>
                    </div>

                    <div class="node-info-row">
                        <span>OS</span>
                        <strong>
                            ${escapeHtml(node.os)}
                        </strong>
                    </div>

                    <div class="node-info-row">
                        <span>Architecture</span>
                        <strong>
                            ${escapeHtml(node.architecture)}
                        </strong>
                    </div>

                    <div class="node-info-row">
                        <span>Agent</span>
                        <strong>
                            ${escapeHtml(node.agent_version)}
                        </strong>
                    </div>

                    <div class="node-info-row">
                        <span>Last Seen</span>
                        <strong>
                            ${formatTime(node.last_seen)}
                        </strong>
                    </div>

                </div>

            </div>

        `).join("");
}


/*
    Select a node.
*/

async function selectNode(nodeId) {

    selectedNode = nodeId;

    document
        .getElementById("nodeDetails")
        .classList.remove("hidden");


    try {

        const node = await api(
            `/api/nodes/${encodeURIComponent(nodeId)}`
        );


        document.getElementById(
            "selectedNodeName"
        ).textContent = node.node_id;


        document.getElementById(
            "selectedNodeInfo"
        ).textContent =
            `${node.hostname} • ${node.os} • ${node.architecture}`;


        await loadHistory();

    } catch (error) {

        console.error(error);

    }
}


/*
    Get historical metrics.

    GET /api/nodes/{node_id}/metrics/history?range=5m
*/

async function loadHistory() {

    if (!selectedNode) {
        return;
    }


    const range =
        document.getElementById(
            "historyRange"
        ).value;


    try {

        const response = await api(
            `/api/nodes/${encodeURIComponent(selectedNode)}` +
            `/metrics/history?range=${range}`
        );


        historyData =
            response.metrics || [];


        updateCurrentMetrics();

        drawCpuChart();

    } catch (error) {

        console.error(error);

    }
}


/*
    Use the newest telemetry record
    as the current metric snapshot.
*/

function updateCurrentMetrics() {

    if (
        !historyData ||
        historyData.length === 0
    ) {

        document.getElementById(
            "cpuValue"
        ).textContent = "-";

        return;
    }


    const latest =
        historyData[
        historyData.length - 1
        ];


    document.getElementById(
        "cpuValue"
    ).textContent =
        `${latest.cpu_usage.toFixed(2)}%`;


    document.getElementById(
        "memoryValue"
    ).textContent =
        `${formatBytes(latest.memory_used)} / ` +
        `${formatBytes(latest.memory_total)}`;


    document.getElementById(
        "diskValue"
    ).textContent =
        `${formatBytes(latest.disk_used)} / ` +
        `${formatBytes(latest.disk_total)}`;


    document.getElementById(
        "networkRx"
    ).textContent =
        `${formatBytes(latest.network_received)}/s`;


    document.getElementById(
        "networkTx"
    ).textContent =
        `${formatBytes(latest.network_transmitted)}/s`;
}


/*
    Draw a simple CPU graph.

    No external chart library yet.
*/

function drawCpuChart() {

    const canvas =
        document.getElementById(
            "cpuChart"
        );


    const ctx =
        canvas.getContext("2d");


    const width =
        canvas.clientWidth;


    const height =
        canvas.clientHeight;


    const ratio =
        window.devicePixelRatio || 1;


    canvas.width =
        width * ratio;


    canvas.height =
        height * ratio;


    ctx.scale(ratio, ratio);


    ctx.clearRect(
        0,
        0,
        width,
        height
    );


    if (
        !historyData ||
        historyData.length < 2
    ) {

        ctx.fillStyle = "#667085";

        ctx.font = "13px Arial";

        ctx.fillText(
            "Not enough telemetry data",
            20,
            30
        );

        return;
    }


    const padding = 30;

    const graphWidth =
        width - padding * 2;

    const graphHeight =
        height - padding * 2;


    /*
        Grid
    */

    ctx.strokeStyle =
        "#1c2532";

    ctx.lineWidth = 1;


    for (
        let i = 0;
        i <= 4;
        i++
    ) {

        const y =
            padding +
            graphHeight -
            (graphHeight * i / 4);


        ctx.beginPath();

        ctx.moveTo(
            padding,
            y
        );

        ctx.lineTo(
            width - padding,
            y
        );

        ctx.stroke();

    }


    /*
        CPU line
    */

    ctx.strokeStyle =
        "#00e5ff";

    ctx.lineWidth = 2;

    ctx.beginPath();


    historyData.forEach(
        (metric, index) => {

            const x =
                padding +
                (
                    index /
                    (historyData.length - 1)
                ) *
                graphWidth;


            const cpu =
                Math.max(
                    0,
                    Math.min(
                        100,
                        metric.cpu_usage
                    )
                );


            const y =
                padding +
                graphHeight -
                (
                    cpu /
                    100 *
                    graphHeight
                );


            if (index === 0) {

                ctx.moveTo(
                    x,
                    y
                );

            } else {

                ctx.lineTo(
                    x,
                    y
                );

            }

        }
    );


    ctx.stroke();


    /*
        Y-axis labels
    */

    ctx.fillStyle =
        "#687487";

    ctx.font =
        "11px Arial";


    ctx.fillText(
        "100%",
        0,
        padding + 4
    );


    ctx.fillText(
        "50%",
        8,
        padding +
        graphHeight / 2 +
        4
    );


    ctx.fillText(
        "0%",
        14,
        height - padding
    );
}


/*
    Connection indicator.
*/

function setConnection(connected) {

    const status =
        document.getElementById(
            "connectionStatus"
        );


    status.textContent =
        connected
            ? "Connected"
            : "Disconnected";
}


/*
    Format bytes.
*/

function formatBytes(bytes) {

    if (
        bytes === null ||
        bytes === undefined
    ) {

        return "-";

    }


    if (bytes === 0) {
        return "0 B";
    }


    const units = [
        "B",
        "KB",
        "MB",
        "GB",
        "TB"
    ];


    const i =
        Math.floor(
            Math.log(bytes) /
            Math.log(1024)
        );


    return (
        bytes /
        Math.pow(1024, i)
    ).toFixed(1)
        + " "
        + units[i];
}


/*
    Convert UTC timestamps
    to the user's local time.
*/

function formatTime(timestamp) {

    return new Date(
        timestamp
    ).toLocaleString();
}


/*
    Basic HTML escaping.
*/

function escapeHtml(value) {

    return String(value)
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;")
        .replaceAll("'", "&#039;");
}


/*
    Events
*/

document
    .getElementById("refreshButton")
    .addEventListener(
        "click",
        loadNodes
    );


document
    .getElementById("closeNode")
    .addEventListener(
        "click",
        () => {

            selectedNode = null;

            document
                .getElementById("nodeDetails")
                .classList.add("hidden");

        }
    );


document
    .getElementById("historyRange")
    .addEventListener(
        "change",
        loadHistory
    );


/*
    Initial load.
*/

loadNodes();


/*
    Automatically refresh node
    information every 5 seconds.
*/

setInterval(
    loadNodes,
    5000
);