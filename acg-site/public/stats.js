const ctx = document.getElementById("myChart");
const colors = [
  '#444444', '#36a2eb',
  '#ff6384', '#ff9f40',
  '#ffcd56', '#4bc0c0',
  '#9966ff', '#c9cbcf'
];
let chart;
function seasonToNum(seasonString) {
  let [s, y] = seasonString.split(" ");
  let sx = ["Winter", "Spring", "Summer", "Fall"].indexOf(s);
  let year = Number(y);
  if (sx == -1 || isNaN(year)) {
     return -1;
  }
  return year * 4 + sx;
}
function numToSeason(seasonNum) {
  if (seasonNum == -1) {
    return "Unknown";
  }
  let y = Math.floor(seasonNum / 4);
  let s = ["Winter", "Spring", "Summer", "Fall"][seasonNum % 4];
  return s + " " + y;
}

function wilsonScoreInterval(p, n) {
  const z = 1.96;
  const a = p + z * z / (2 * n);
  const b = z * Math.sqrt((p * (1 - p) + z * z / (4 * n)) / n);
  const c = 1 + z * z / n;
  return [(a - b)/c, (a + b)/c];
}

function isCI(dataset) {
  return dataset.linetype && dataset.linetype.startsWith("CI");
}

function pointScale(c) {
  return Math.log10(c) + 1
}

function createDatasets(kinds) {
  let datasets = [];
  for (const index in kinds) {
    const [kind, label_gr, label_tc] = kinds[index];
    let col = colors[index % colors.length];
    let dataset_index = datasets.length;
    if (label_gr) {
      datasets.push({
        kind: kind,
        type: "line",
        hidden: true,
        label: label_gr,
        data: [],
        pointRadius: [],
        borderWidth: 2,
        borderColor: col,
        backgroundColor: col + "80",
        parsing: { yAxisKey: "y" },
        yAxisID: "ygr"
      });
    }
    if (label_tc) {
      datasets.push({
        kind: kind,
        type: "bar",
        label: label_tc,
        data: [],
        borderColor: col,
        backgroundColor: col + "80",
        parsing: { yAxisKey: "c" },
        stack: 0,
        yAxisID: "yc"
      });
    }
    datasets.push({
      kind: kind,
      linetype: "CIupper",
      type: "line",
      data: [],
      pointRadius: 0,
      fill: dataset_index,
      borderColor: "transparent",
      backgroundColor: col + "80",
      parsing: { yAxisKey: "y" },
      yAxisID: "ygr"
    });
    datasets.push({
      kind: kind,
      linetype: "CIlower",
      type: "line",
      data: [],
      pointRadius: 0,
      fill: dataset_index,
      borderColor: "transparent",
      backgroundColor: col + "80",
      parsing: { yAxisKey: "y" },
      yAxisID: "ygr"
    });
  }
  return datasets;
}

function getVintageStats() {
  const zoomOptions = {
    pan: {
      enabled: true,
      mode: 'x',
    },
    zoom: {
      wheel: {
        enabled: true,
      },
      pinch: {
        enabled: true
      },
      mode: 'x',
    }
  };
  const tooltipOptions = {
    callbacks: {
      afterLabel: ctx => ctx.raw.z ? ctx.raw.z + " songs" : undefined,
      afterBody: items => "Total plays: " + items.reduce((a,x) => (a+x.raw.c||0), 0),
    },
    filter: item => !isCI(item.dataset)
  };
  const legendOptions = {
    display: true,
    labels: {
       filter: (item, data) => {
           return !isCI(data.datasets[item.datasetIndex]);
       }
    }
  };
  if (chart) {
    chart.destroy();
  }
  let kinds = [
    ["All", "Guess Rate (All)", null],
    ["Opening", "Guess Rate (Openings)", "Total Plays (Opening)"],
    ["Ending", "Guess Rate (Endings)", "Total Plays (Ending)"],
    ["Insert", "Guess Rate (Inserts)", "Total Plays (Insert)"]
  ];
  let datasets = createDatasets(kinds);
  chart = new Chart(ctx, {
    data: {
      labels: [],
      datasets: datasets
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      spanGaps: true,
      interaction: {
        mode: "x",
      },
      scales: {
        y: { display: false },
        ygr: {
          type: "linear",
          beginAtZero: true,
          position: "left",
          min: 0,
          max: 1.05,
          ticks: {
            includeBounds: false,
          }
        },
        yc: {
          type: "linear",
          beginAtZero: true,
          stacked: true,
          position: "right",
          grid: {
            drawOnChartArea: false, // only want the grid lines for one axis to show up
          },
        },
      },
      plugins: {
        zoom: zoomOptions,
        tooltip: tooltipOptions,
        legend: legendOptions,
      }
    }
  });

  datasets[0].hidden = false;

  function handleData(rows) {
    if (rows.length == 0) {
      return;
    }
    rows.sort((a, b) => seasonToNum(b.vintage) - seasonToNum(a.vintage));
    let allAvg = {};
    rows.forEach((el, index) => {
      const point = {x: el.vintage, y: el.guess_rate, z: el.guess_count, c: el.times_played};
      // wilson score interval
      const [lower, upper] = wilsonScoreInterval(point.y, point.z);
      let s = seasonToNum(point.x);
      chart.data.datasets.forEach((dataset) => {
        if (dataset.kind == el.kind) {
          if (dataset.linetype == "CIupper") {
            dataset.data.push({x: el.vintage, y: upper});
          } else if (dataset.linetype == "CIlower") {
            dataset.data.push({x: el.vintage, y: lower});
          } else {
            dataset.data.push(point);
            if (dataset.pointRadius) {
              dataset.pointRadius.push(pointScale(point.z));
            }
          }
        }
      });
    });

    let maxSeason = seasonToNum(rows[0].vintage);
    let minSeason = seasonToNum(rows[rows.length - 1].vintage);
    chart.data.labels = Array.from(new Array(maxSeason - minSeason + 1), (x, i) => numToSeason(i + minSeason));
    let minScale = Math.max(minSeason, seasonToNum("Winter 2007")) - minSeason;
    let maxScale = Math.min(maxSeason, seasonToNum("Fall 2020")) - minSeason;
    chart.zoomScale('x', {min: minScale, max: maxScale});
    chart.update();
  }

  fetch("/stats/vintage", {
      method: "GET",
      headers: {
          "Accept": "application/json",
      },
  })
  .then(res => {
    if(!res.ok) {
      return res.text().then(text => { throw new Error(text) })
    } else {
      return res.json();
    }
  })
  .then(response => handleData(response))
  .catch(err => alert(err));
}

function getDifficultyStats(event) {
  let numBins = Number(document.getElementById("inputNumBins").value);
  const zoomOptions = {
    pan: {
      enabled: true,
      mode: 'x',
    },
    limits: {
      x: { min: 0, max: 100 }
    },
    zoom: {
      wheel: {
        enabled: true,
      },
      pinch: {
        enabled: true
      },
      mode: 'x',
    }
  };
  const tooltipOptions = {
    callbacks: {
      afterLabel: ctx => ctx.raw.z ? ctx.raw.z + " songs" : undefined,
      afterBody: items => "Total plays: " + items.reduce((a,x) => (a+x.raw.c||0), 0),
    },
    filter: item => !isCI(item.dataset)
  };
  const legendOptions = {
    display: true,
    labels: {
       filter: (item, data) => {
           return !isCI(data.datasets[item.datasetIndex]);
       }
    }
  };

  if (chart) {
    chart.destroy();
  }
  chart = new Chart(ctx, {
    data: {
      datasets: [{
        type: "line",
        label: "Guess Rate",
        data: [],
        borderColor: colors[0],
        backgroundColor: colors[0] + "FF",
        borderWidth: 2,
      },
      {
        linetype: "CIupper",
        type: "line",
        data: [],
        fill: 0,
        borderColor: "transparent",
        backgroundColor: colors[0] + "80",
        borderWidth: 0,
        pointRadius: 0,
      },
      {
        linetype: "CIlower",
        type: "line",
        data: [],
        fill: 0,
        borderColor: "transparent",
        backgroundColor: colors[0] + "80",
        borderWidth: 0,
        pointRadius: 0,
      },
      {
        type: "bar",
        label: "Total Plays",
        data: [],
        borderColor: colors[1],
        backgroundColor: colors[1] + "80",
        yAxisID: "y1"
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      spanGaps: true,
      scales: {
        y: {
          beginAtZero: true,
          position: "left",
        },
        y1: {
          beginAtZero: true,
          position: "right",
          grid: {
            drawOnChartArea: false,
          },
        },
        x: {
          type: "linear",
          min: 0.0,
          max: 100.0,
        }
      },
      plugins: {
        zoom: zoomOptions,
        tooltip: tooltipOptions,
        legend: legendOptions,
      }
    }
  });

  // diff_bin is 1-indexed
  function handleData(rows, bins) {
    // rows need to be in reverse order for some reason
    // to display the lines properly
    // rows.reverse();
    rows.forEach((row) => {
      let diff = (row.diff_bin - 0.5)/bins * 100;
      let guess_rate = row.guess_rate === null ? null : row.guess_rate * 100;
      let [lower, upper] = wilsonScoreInterval(row.guess_rate, row.guess_count);
      chart.data.datasets.forEach(dataset => {
        if (dataset.linetype == "CIupper") {
          dataset.data.push({ x: diff, y: upper * 100 });
        } else if (dataset.linetype == "CIlower") {
          dataset.data.push({ x: diff, y: lower * 100 });
        } else if (dataset.type == "bar") {
          dataset.data.push({ x: diff, y: row.times_played });
        } else {
          dataset.data.push({ x: diff, y: guess_rate, z: row.guess_count });
        }
      });
    });
    chart.update();
  }

  // fetch("/stats/difficulty" + new URLSearchParams({ "bins": numBins }), {
  fetch("/stats/difficulty/" + numBins, {
      method: "GET",
      headers: {
          "Accept": "application/json",
      },
  })
  .then(res => {
    if(!res.ok) {
      return res.text().then(text => { throw new Error(text) })
    } else {
      return res.json();
    }
  })
  .then(response => handleData(response, numBins))
  .catch(err => alert(err));
}

function getDifficultyStats2(event) {
  let numBins = Number(document.getElementById("inputNumBins").value);
  const zoomOptions = {
    pan: {
      enabled: true,
      mode: 'x',
    },
    limits: {
      x: { min: 0, max: 100 }
    },
    zoom: {
      wheel: {
        enabled: true,
      },
      pinch: {
        enabled: true
      },
      mode: 'x',
    }
  };
  const tooltipOptions = {
    callbacks: {
      afterLabel: ctx => ctx.raw.z ? ctx.raw.z + " songs" : undefined,
      afterBody: items => "Total plays: " + items.reduce((a,x) => (a+x.raw.c||0), 0),
    },
    filter: item => !isCI(item.dataset)
  };
  const legendOptions = {
    display: true,
    labels: {
       filter: (item, data) => {
           return !isCI(data.datasets[item.datasetIndex]);
       }
    }
  };

  if (chart) {
    chart.destroy();
  }
  let kinds = [
    ["All", "Guess Rate", null],
    ["Opening", "Guess Rate (Openings)", null],
    ["Ending", "Guess Rate (Endings)", null],
    ["Insert", "Guess Rate (Inserts)", null]
  ];
  let datasets = createDatasets(kinds);
  chart = new Chart(ctx, {
    data: {
      datasets: datasets,
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      spanGaps: true,
      scales: {
        y: { display: false },
        ygr: {
          type: "linear",
          beginAtZero: true,
          position: "left",
          min: 0,
          max: 101,
          ticks: {
            includeBounds: false,
          }
        },
        x: {
          type: "linear",
          min: 0.0,
          max: 100.0,
        }
      },
      plugins: {
        zoom: zoomOptions,
        tooltip: tooltipOptions,
        legend: legendOptions,
      }
    }
  });

  // diff_bin is 1-indexed
  function handleData(rows, bins) {
    rows.reverse();
    rows.forEach((row) => {
      let diff = (row.bucket_min + row.bucket_max) / 2;
      let guess_rate = row.guess_rate === null ? null : row.guess_rate * 100;
      let [lower, upper] = wilsonScoreInterval(row.guess_rate, row.guess_count);
      chart.data.datasets.forEach(dataset => {
        if (dataset.kind == row.kind) {
          if (dataset.linetype == "CIupper") {
            dataset.data.push({ x: diff, y: upper * 100 });
          } else if (dataset.linetype == "CIlower") {
            dataset.data.push({ x: diff, y: lower * 100 });
          } else {
            let point = { x: diff, y: guess_rate, z: row.guess_count };
            dataset.data.push(point);
            if (dataset.pointRadius) {
              dataset.pointRadius.push(pointScale(point.z));
            } else {
              dataset.pointRadius = 3;
            }
          }
        }
      });
    });
    chart.update();
  }

  fetch("/stats/difficulty2/" + numBins, {
      method: "GET",
      headers: {
          "Accept": "application/json",
      },
  })
  .then(res => {
    if(!res.ok) {
      return res.text().then(text => { throw new Error(text) })
    } else {
      return res.json();
    }
  })
  .then(response => handleData(response, numBins))
  .catch(err => alert(err));
}


document.getElementById("btnVintage").addEventListener("click", getVintageStats);
document.getElementById("btnDifficulty").addEventListener("click", getDifficultyStats);
document.getElementById("btnDifficulty2").addEventListener("click", getDifficultyStats2);
