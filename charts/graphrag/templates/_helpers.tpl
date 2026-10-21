{{/*
Expand the name of the chart.
*/}}
{{- define "graphrag.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "graphrag.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "graphrag.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{- define "graphrag.config" -}}
{{- $baseconfig := fromYaml (.Files.Get "configs/config.yaml") }}
{{- $newconfig := default dict .Values.config }}
{{- $postmerge := mergeOverwrite $baseconfig $newconfig }}
{{- tpl (toYaml $postmerge) . }}
{{- end -}}

{{- define "graphrag.volumes" -}}
{{- tpl (toYaml .Values.volumes) . }}
{{- end -}}

{{- define "graphrag.volumeMounts" -}}
{{- tpl (toYaml .Values.volumeMounts) . }}
{{- end -}}

{{- define "graphrag.env" -}}
{{- tpl (toYaml .Values.env) . }}
{{- end -}}

{{/*
Common labels
*/}}
{{- define "graphrag.labels" -}}
helm.sh/chart: {{ include "graphrag.chart" . }}
{{ include "graphrag.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "graphrag.selectorLabels" -}}
app.kubernetes.io/name: {{ include "graphrag.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app: {{ include "graphrag.name" . }}
{{- end }}
