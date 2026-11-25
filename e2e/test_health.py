import requests


def test_health(service):
    # 200 only when the indexer reaches its database
    r = requests.get(service("indexer", 8000) + "/health", timeout=10)
    assert r.status_code == 200


def test_manage_info(service):
    # what runs here: version, commit, branch and build time of the image
    r = requests.get(service("indexer", 8000) + "/manage/info", timeout=10)
    assert r.status_code == 200
    info = r.json()
    assert info["service"] == "pouw-indexer"
    assert {"version", "commit", "branch", "build_time"} <= info.keys()
