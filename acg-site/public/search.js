let table = $("#songs").DataTable({
  dom: "<<t>ip>",
  columns: [
    {data: 1, className: "dt-left dt-control"},
    {data: 2},
    {data: 3},
    {data: 4},
  ],
  pageLength: 100,
  search: {
    return: true
  },
  stateSave: true,
});

function format(arr) {
  let col0 = [];
  let col2 = [];
  arr.sort((a, b) => a[0] > b[0] ? 1 : -1);
  arr.forEach((e) => {
    if (e[0] == "id") {
      col0.push("ANN ID: " + e[1])
    } else if (e[0] == "mp3") {
      col0.push($("<a>").attr("href", "//files.catbox.moe/" + e[1]).append("Sound"));
    } else if (e[0] == "video") {
      col0.push($("<a>").attr("href", "//files.catbox.moe/" + e[1]).append("Video"));
    } else if (e[0] == "name") {
      col2.push(e[1]);
    }
  });
  let rows = [];
  for(let i = 0; i < Math.max(col0.length, col2.length); i++) {
    rows.push($("<tr>")
    .append($("<td>").append(col0[i]))
    .append($("<td>"))
    .append($("<td>").append(col2[i]))
    .append($("<td>")));
  }
  return rows;
}

const songData = new Map();

table.on("click", "td.dt-control", function (e) {
  let tr = e.target.closest("tr");
  let row = table.row(tr);
  let songId = row.data()[0];
  if (row.child.isShown()) {
    row.child.hide();
  } else {
    let data = songData.get(songId);
    if (data === undefined) {
      $("#loader").show();
      fetch("/songquery/" + songId, {
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
      .then(response => {
        songData.set(songId, response);
        row.child(format(response)).show();
        table.columns.adjust();
        $("#loader").hide();
      })
      .catch(err => {
        alert(err);
        $("#loader").hide();
      });
    } else {
      row.child(format(data)).show();
      table.columns.adjust();
    }
  }
});

$("#divSearchBox input").on("keypress", (e) => {
  if (e.keyCode !== 13) {
    return;
  }
  let search = $(e.target).val();
  if (search == "") {
    return;
  }
  table.clear();
  $("#loader").show();
  let params = new URLSearchParams({
    "search": search,
    "exact": $("#chkExactMatch").prop("checked"),
  });
  fetch("/query?" + params, {
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
  .then(response => {
    table.rows.add(response).draw();
    table.columns.adjust();
    $("#loader").hide();
  })
  .catch(err => {
    alert(err);
    $("#loader").hide();
  });
});

