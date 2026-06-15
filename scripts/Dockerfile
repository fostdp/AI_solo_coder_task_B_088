FROM python:3.11-slim

WORKDIR /app

RUN pip install --no-cache-dir requests paho-mqtt

COPY tiantan_simulator.py .

CMD ["python", "tiantan_simulator.py"]
