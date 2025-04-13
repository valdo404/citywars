name := "clickplanet-osm"
organization := "com.clickplanet"
version := "0.1.0"

scalaVersion := "2.12.20"

val sparkVersion = "3.5.5"

resolvers ++= Seq(
  "Maven Central" at "https://repo1.maven.org/maven2/",
  "Sonatype OSS Snapshots" at "https://oss.sonatype.org/content/repositories/snapshots",
  "Sonatype Releases" at "https://oss.sonatype.org/content/repositories/releases"
)

libraryDependencies ++= Seq(
  "org.apache.spark" %% "spark-core" % sparkVersion,
  "org.apache.spark" %% "spark-sql" % sparkVersion,
  // Using the regular libraries with shading rules applied
  "io.github.valdo404" %% "osm4scala-core" % "1.0.11",
  "io.github.valdo404" %% "osm4scala-spark3" % "1.0.11",
  "org.locationtech.jts" % "jts-core" % "1.18.2",
  "org.postgresql" % "postgresql" % "42.5.4",
  "org.rogach" %% "scallop" % "4.1.0", // Command line parser
  "com.typesafe.scala-logging" %% "scala-logging" % "3.9.5",
  "ch.qos.logback" % "logback-classic" % "1.2.12"
)

// Configure proper shading rules to avoid protobuf conflicts
ThisBuild / assemblyShadeRules := Seq(
  ShadeRule.rename("com.google.protobuf.**" -> "shaded.com.google.protobuf.@1").inAll,
  ShadeRule.rename("scalapb.**" -> "shaded.scalapb.@1").inAll
)

// Assembly settings for creating a fat JAR with proper class relocation
assembly / assemblyMergeStrategy := {
  case PathList("META-INF", xs @ _*) => MergeStrategy.discard
  case "reference.conf" => MergeStrategy.concat
  case x => MergeStrategy.first
}

// Set debug level to see shading details
assembly / logLevel := Level.Debug

// Don't include Spark libraries in the fat JAR
assembly / assemblyExcludedJars := {
  val cp = (assembly / fullClasspath).value
  cp filter { f =>
    f.data.getName.contains("spark-core") || 
    f.data.getName.contains("spark-sql") ||
    f.data.getName.contains("scala-library")
  }
}

// Assembly JAR filename
assembly / assemblyJarName := s"${name.value}-${version.value}.jar"
