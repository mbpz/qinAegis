# sandbox/locustfile.py
from locust import HttpUser, task, between

class WebsiteUser(HttpUser):
    wait_time = between(1, 3)

    @task
    def index_page(self):
        self.client.get("/")

    @task
    def health_check(self):
        self.client.get("/health")
