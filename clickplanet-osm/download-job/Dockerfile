FROM python:3.11-slim

# Install aria2
RUN apt-get update && \
    apt-get install -y aria2 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY requirements.txt .
RUN pip install -r requirements.txt

COPY . .

CMD ["python", "main.py"]
